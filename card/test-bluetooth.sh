#!/bin/sh
set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
SCRIPT="$HERE/System/bluetooth.sh"
TMP="$(mktemp -d)"
trap 'stray; rm -rf "$TMP"' EXIT HUP INT TERM
FAILS=0

fail() {
	echo "FAIL: $*"
	FAILS=$((FAILS + 1))
}

# The fake rtk_hciattach and the discovery bluetoothctl are processes standing in for real
# ones, so the script can stop them. Any left at the end of a case are stopped here, and gone
# before the next starts, so none tidies up after itself into the next one's files.
stray() {
	for p in "$TMP"/run/*.pid "$TMP"/hciattach.pid; do
		[ -s "$p" ] || continue
		pid="$(cat "$p")"
		kill "$pid" 2>/dev/null || continue
		n=0
		while kill -0 "$pid" 2>/dev/null && [ "$n" -lt 20 ]; do
			sleep 0.1
			n=$((n + 1))
		done
	done
	return 0
}

# A device's `info`, as bluetoothctl prints it, into the file $1; then its address, name, icon,
# whether it is paired, trusted and connected, and its alias: the one given, else its name, else,
# as bluetoothd has it for a device that gave none, its address with dashes.
info() {
	at="$1" mac="$2" name="$3" icon="$4" paired="$5" trusted="$6" connected="$7"
	alias="${8:-${name:-$(echo "$mac" | tr : -)}}"
	{
		echo "Device $mac (public)"
		[ -n "$name" ] && printf '\tName: %s\n' "$name"
		printf '\tAlias: %s\n' "$alias"
		[ -n "$icon" ] && printf '\tIcon: %s\n' "$icon"
		printf '\tPaired: %s\n\tBonded: %s\n\tTrusted: %s\n\tBlocked: no\n\tConnected: %s\n' \
			"$paired" "$paired" "$trusted" "$connected"
		printf '\tLegacyPairing: no\n'
	} > "$at"
}

# A device bluetoothd knows: its `info`, and its line in `devices`, which names it by its alias.
known() {
	info "$TMP/info/$1" "$@"
	echo "Device $1 $alias" >> "$TMP/devices"
}

# A device in reach that bluetoothd has forgotten, or not found yet: a discovery finds it.
reach() {
	info "$TMP/reach/$1" "$@"
}

# bluetoothctl: each verb as the real one answers it, against the files under $TMP. Given
# --agent it is the pairing session, reading its commands from stdin.
fake_bluetoothctl() {
	cat > "$TMP/bin/bluetoothctl" <<EOF
#!/bin/sh
T="$TMP"
printf '%s\n' "\$*" >> "\$T/bluetoothctl.log"
set_prop() {
	[ -f "\$T/info/\$1" ] || return 1
	sed "s/^\\(	\$2:\\) .*/\\1 \$3/" "\$T/info/\$1" > "\$T/info/\$1.new"
	mv "\$T/info/\$1.new" "\$T/info/\$1"
}
case "\$1" in
show)
	echo 'Controller 68:8F:C9:3C:2D:6D (public)'
	if [ -e "\$T/powered" ]; then echo '	Powered: yes'; else echo '	Powered: no'; fi
	if [ -e "\$T/discovering" ]; then echo '	Discovering: yes'; else echo '	Discovering: no'; fi
	;;
devices) cat "\$T/devices" 2>/dev/null ;;
info) [ -f "\$T/info/\$2" ] && cat "\$T/info/\$2" || { echo "Device \$2 not available"; exit 1; } ;;
power) : > "\$T/powered" ;;
trust) set_prop "\$2" Trusted yes ;;
connect) [ -z "\${FAKE_CONNECT_FAIL:-}" ] && set_prop "\$2" Connected yes || exit 1 ;;
disconnect) set_prop "\$2" Connected no ;;
remove)
	rm -f "\$T/info/\$2"
	grep -v " \$2 " "\$T/devices" > "\$T/devices.new" || true
	mv "\$T/devices.new" "\$T/devices"
	;;
--timeout)
	: > "\$T/discovering"
	trap 'rm -f "\$T/discovering"; exit 0' TERM
	n=0
	while [ "\$n" -lt "\$((\$2 * 10))" ]; do sleep 0.1; n=\$((n + 1)); done
	rm -f "\$T/discovering"
	;;
--agent)
	[ -e "\$T/discovering" ] && echo 'paired while discovering' >> "\$T/agent.log"
	echo 'Agent registered'
	while IFS= read -r line; do
		printf '%s\n' "\$line" >> "\$T/agent.log"
		set -- \$line
		case "\${1:-}" in
		default-agent) echo 'Default agent request successful' ;;
		scan)
			[ "\${2:-}" = on ] || continue
			for f in "\$T"/reach/*; do
				[ -f "\$f" ] || continue
				m="\$(basename "\$f")"
				mv "\$f" "\$T/info/\$m"
				echo "Device \$m \$(sed -n 's/^	Alias: //p' "\$T/info/\$m")" >> "\$T/devices"
			done
			echo 'Discovery started'
			;;
		pair)
			if [ ! -f "\$T/info/\$2" ]; then echo "Device \$2 not available"; continue; fi
			if [ -n "\${FAKE_PAIR_FAIL:-}" ]; then
				echo 'Failed to pair: org.bluez.Error.AuthenticationFailed'
				continue
			fi
			if [ -n "\${FAKE_ASK:-}" ]; then
				echo '[agent] Authorize service 00001124-0000-1000-8000-00805f9b34fb (yes/no): '
				IFS= read -r answer
				printf '%s\n' "\$answer" >> "\$T/agent.log"
				[ "\$answer" = yes ] || continue
			fi
			if [ -n "\${FAKE_PIN:-}" ]; then
				echo '[agent] Enter PIN code: '
				IFS= read -r answer
				printf '%s\n' "\$answer" >> "\$T/agent.log"
			fi
			set_prop "\$2" Paired yes
			echo 'Pairing successful'
			;;
		trust) set_prop "\$2" Trusted yes ;;
		connect)
			if [ -n "\${FAKE_CONNECT_FAIL:-}" ]; then
				echo 'Failed to connect: org.bluez.Error.Failed br-connection-page-timeout'
			else
				set_prop "\$2" Connected yes
				echo 'Connection successful'
			fi
			;;
		quit) exit 0 ;;
		esac
	done
	;;
esac
exit 0
EOF
	chmod 755 "$TMP/bin/bluetoothctl"
}

fake() {
	name="$1"
	shift
	{
		echo '#!/bin/sh'
		echo "printf '%s\\n' \"\$*\" >> \"$TMP/$name.log\""
		for line in "$@"; do echo "$line"; done
		echo 'exit 0'
	} > "$TMP/bin/$name"
	chmod 755 "$TMP/bin/$name"
}

reset() {
	stray
	rm -rf "$TMP/run" "$TMP/rfkill" "$TMP/sys" "$TMP/info" "$TMP/reach" "$TMP/bin" "$TMP"/*.log \
		"$TMP/devices" "$TMP/powered" "$TMP/discovering" "$TMP/bluetoothd" "$TMP/hciattach.pid"
	mkdir -p "$TMP/run" "$TMP/rfkill/rfkill0" "$TMP/rfkill/rfkill1" "$TMP/sys" "$TMP/info" \
		"$TMP/reach" "$TMP/bin"
	echo wlan > "$TMP/rfkill/rfkill0/type"
	echo 0 > "$TMP/rfkill/rfkill0/soft"
	echo bluetooth > "$TMP/rfkill/rfkill1/type"
	echo 1 > "$TMP/rfkill/rfkill1/soft"
	: > "$TMP/devices"
	fake rtk_hciattach \
		"echo \$\$ > \"$TMP/hciattach.pid\"" \
		"[ -n \"\${FAKE_NO_HCI:-}\" ] || mkdir -p \"$TMP/sys/hci0\"" \
		"trap 'rm -rf \"$TMP/sys/hci0\"; exit 0' TERM" \
		'while :; do sleep 0.1; done'
	fake hciconfig
	fake systemctl \
		'case "$1" in' \
		"start) : > \"$TMP/bluetoothd\"; [ -n \"\${FAKE_UNPOWERED:-}\" ] || : > \"$TMP/powered\" ;;" \
		"stop) rm -f \"$TMP/bluetoothd\" \"$TMP/powered\" ;;" \
		"is-active) [ -e \"$TMP/bluetoothd\" ] || exit 3 ;;" \
		'esac'
	fake timeout 'shift' 'exec "$@"'
	fake pidof \
		"[ \"\$1\" = rtk_hciattach ] && [ -s \"$TMP/hciattach.pid\" ] && kill -0 \"\$(cat \"$TMP/hciattach.pid\")\" 2>/dev/null || exit 1" \
		"cat \"$TMP/hciattach.pid\""
	fake_bluetoothctl
}

run() {
	PATH="$TMP/bin:$PATH" \
		AGS_RUN="$TMP/run" AGS_RFKILL_SYS="$TMP/rfkill" AGS_BT_SYS="$TMP/sys" \
		AGS_BT_WAIT_S="${WAIT_S:-1}" AGS_BT_SCAN_S="${SCAN_S:-30}" AGS_BT_FIND_S=1 AGS_BT_PAIR_S=2 \
		AGS_BT_CONNECT_S=2 AGS_BT_CTL_S=2 \
		AGS_RTK_HCIATTACH="$TMP/bin/rtk_hciattach" AGS_HCICONFIG="$TMP/bin/hciconfig" \
		AGS_BLUETOOTHCTL="$TMP/bin/bluetoothctl" AGS_SYSTEMCTL="$TMP/bin/systemctl" \
		AGS_TIMEOUT="$TMP/bin/timeout" AGS_PIDOF="$TMP/bin/pidof" \
		AGS_BLUEALSA="$TMP/bin/bluealsa" \
		sh "$SCRIPT" "$@"
}

# Discovery is a background process: give it a moment to start or stop.
settle() {
	n=0
	while [ "$n" -lt 20 ]; do
		if [ "$1" = gone ]; then [ ! -e "$2" ] && return 0; else [ -e "$2" ] && return 0; fi
		sleep 0.1
		n=$((n + 1))
	done
	return 1
}

up() {
	run up >/dev/null || fail "up did not bring Bluetooth up"
}

PAD=8C:EA:48:12:34:56
CANS=38:18:4C:AB:CD:EF
KEYS=DC:2C:26:01:02:03
ODD=4C:4C:6B:1A:80:D3

reset
run up && rc=0 || rc=$?
[ "$rc" = 0 ] || fail "up exited $rc"
[ -f "$TMP/run/omega-bt.up" ] || fail "up left no mark that Bluetooth is wanted"
[ "$(cat "$TMP/rfkill/rfkill1/soft")" = 0 ] || fail "up left the radio blocked"
[ "$(cat "$TMP/rfkill/rfkill0/soft")" = 0 ] || fail "up touched Wi-Fi's switch"
grep -qx -- '-n -s 115200 /dev/ttyS1 rtk_h5' "$TMP/rtk_hciattach.log" \
	|| fail "the firmware was not loaded over the UART"
[ -s "$TMP/run/omega-bt-attach.pid" ] || fail "up did not keep rtk_hciattach's pid"
grep -qx 'hci0 up' "$TMP/hciconfig.log" || fail "up did not bring hci0 up"
grep -qx 'start bluetooth' "$TMP/systemctl.log" || fail "up did not start bluetoothd"
grep -qx 'power on' "$TMP/bluetoothctl.log" && fail "up powered a controller that AutoEnable had"
run up >/dev/null
[ "$(grep -c rtk_h5 "$TMP/rtk_hciattach.log")" = 1 ] || fail "a second up loaded the firmware again"

# bluez-alsa comes up with bluetoothd, for headphones to have an audio profile to connect to,
# and goes down with it
reset
cat > "$TMP/bin/bluealsa" <<FAKE
#!/bin/sh
printf '%s\n' "\$*" >> "$TMP/bluealsa.log"
exec sleep 30
FAKE
chmod 755 "$TMP/bin/bluealsa"
up
grep -q -- '-p a2dp-source' "$TMP/bluealsa.log" || fail "up did not start bluez-alsa as an A2DP source"
audio="$(cat "$TMP/run/omega-bt-audio.pid")"
kill -0 "$audio" 2>/dev/null || fail "bluez-alsa was not left running"
up
[ "$(grep -c a2dp "$TMP/bluealsa.log")" = 1 ] || fail "a second up started bluez-alsa again"
run down >/dev/null
kill -0 "$audio" 2>/dev/null && fail "down left bluez-alsa running"

reset
FAKE_UNPOWERED=1 run up && rc=0 || rc=$?
[ "$rc" = 0 ] || fail "up with the controller off exited $rc"
grep -qx 'power on' "$TMP/bluetoothctl.log" || fail "up left the controller off"

reset
FAKE_NO_HCI=1 WAIT_S=0 run up && rc=0 || rc=$?
[ "$rc" = 1 ] || fail "up without hci0 exited $rc"
grep -q 'start bluetooth' "$TMP/systemctl.log" 2>/dev/null && fail "bluetoothd started without a radio"

reset
out="$(run status)"
[ "$out" = "$(printf 'enabled=no\npowered=no\ndiscovering=no')" ] || fail "status while down said $out"
[ -e "$TMP/bluetoothctl.log" ] && fail "status asked bluetoothctl with no bluetoothd to answer"

reset
up
known "$PAD" '8BitDo SN30 Pro' input-gaming yes yes yes
known "$CANS" 'WH-1000XM4' audio-headphones yes yes no
known "$KEYS" 'Keychron K3' input-keyboard no no no
known AA:BB:CC:DD:EE:01 'Headset' audio-headset no no no
known AA:BB:CC:DD:EE:02 'Speaker' audio-card no no no
known AA:BB:CC:DD:EE:03 'Phone' phone no no no
# found, and named only by an alias: the alias it was given, but not one that is its address
known AA:BB:CC:DD:EE:04 '' audio-headset no no no 'Pixel Buds'
known "$ODD" '' '' no no no
known 4C:4C:6B:1A:80:D4 '' '' no no no '4C:4C:6B:1A:80:D4'
known 4C:4C:6B:1A:80:D5 '' '' no no no '4c-4c-6b-1a-80-d5'
# a name of its own over the alias it was given
known AA:BB:CC:DD:EE:06 'Mi Smart Band 4' '' no no no 'Band'
# paired with no name: by its alias, whatever it is
known AA:BB:CC:DD:EE:07 '' input-gaming yes yes no
known AA:BB:CC:DD:EE:08 '' input-keyboard yes no no 'Desk'
echo 'Device not-a-mac Junk' >> "$TMP/devices"
out="$(run status)"
tab="$(printf '\t')"
for want in enabled=yes powered=yes discovering=no \
	"device=$PAD${tab}gamepad${tab}yes${tab}yes${tab}yes${tab}8BitDo SN30 Pro" \
	"device=$CANS${tab}audio${tab}yes${tab}yes${tab}no${tab}WH-1000XM4" \
	"device=$KEYS${tab}keyboard${tab}no${tab}no${tab}no${tab}Keychron K3" \
	"device=AA:BB:CC:DD:EE:01${tab}audio${tab}no${tab}no${tab}no${tab}Headset" \
	"device=AA:BB:CC:DD:EE:02${tab}audio${tab}no${tab}no${tab}no${tab}Speaker" \
	"device=AA:BB:CC:DD:EE:03${tab}other${tab}no${tab}no${tab}no${tab}Phone" \
	"device=AA:BB:CC:DD:EE:04${tab}audio${tab}no${tab}no${tab}no${tab}Pixel Buds" \
	"device=AA:BB:CC:DD:EE:06${tab}other${tab}no${tab}no${tab}no${tab}Mi Smart Band 4" \
	"device=AA:BB:CC:DD:EE:07${tab}gamepad${tab}yes${tab}yes${tab}no${tab}AA-BB-CC-DD-EE-07" \
	"device=AA:BB:CC:DD:EE:08${tab}keyboard${tab}yes${tab}no${tab}no${tab}Desk"; do
	printf '%s\n' "$out" | grep -qxF -- "$want" || fail "status lacks $want"
done
for nameless in "$ODD" 4C:4C:6B:1A:80:D4 4C:4C:6B:1A:80:D5; do
	printf '%s\n' "$out" | grep -q "^device=$nameless" && fail "status listed $nameless, found with no name"
done
printf '%s\n' "$out" | grep -q 'Junk' && fail "status passed on a line that is no device"

reset
run scan on && rc=0 || rc=$?
[ "$rc" = 1 ] || fail "scan with Bluetooth down exited $rc"
up
run scan on
settle there "$TMP/discovering" || fail "scan on did not start discovery"
run status | grep -qx discovering=yes || fail "status did not say it is discovering"
run scan on
[ "$(grep -c '^--timeout 30 scan on$' "$TMP/bluetoothctl.log")" = 1 ] || fail "a second scan on started a second discovery"
run scan off
settle gone "$TMP/discovering" || fail "scan off left discovery running"
[ -e "$TMP/run/omega-bt-scan.pid" ] && fail "scan off left its pid"
run scan && rc=0 || rc=$?
[ "$rc" = 2 ] || fail "scan with neither on nor off exited $rc"
SCAN_S=1 run scan on
sleep 1.6
[ -e "$TMP/discovering" ] && fail "discovery outlived its time"

reset
up
known "$KEYS" 'Keychron K3' input-keyboard no no no
run scan on
settle there "$TMP/discovering"
run pair "$KEYS" && rc=0 || rc=$?
[ "$rc" = 0 ] || fail "pair exited $rc"
grep -q 'paired while discovering' "$TMP/agent.log" && fail "pair left discovery running"
[ "$(grep -x -e default-agent -e "pair $KEYS" -e "trust $KEYS" -e "connect $KEYS" -e quit "$TMP/agent.log" | tr '\n' ' ')" \
	= "default-agent pair $KEYS trust $KEYS connect $KEYS quit " ] \
	|| fail "the agent was not asked to pair, trust and connect in turn: $(tr '\n' ' ' < "$TMP/agent.log")"
grep -qx 'scan on' "$TMP/agent.log" && fail "a device bluetoothd knew was looked for again"
grep -qx -- '--agent NoInputNoOutput' "$TMP/bluetoothctl.log" || fail "the pairing had no NoInputNoOutput agent"
for prop in Paired Trusted Connected; do
	grep -qx "	$prop: yes" "$TMP/info/$KEYS" || fail "after pair the device is not $prop"
done
ls "$TMP/run" | grep -q pair && fail "pair left its fifo or log behind"

reset
up
known "$KEYS" 'Keychron K3' input-keyboard no no no
FAKE_ASK=1 run pair "$KEYS" && rc=0 || rc=$?
[ "$rc" = 0 ] || fail "pair whose agent was asked exited $rc"
grep -qx yes "$TMP/agent.log" || fail "the agent did not say yes"

reset
up
known "$KEYS" 'Old Pad' '' no no no
FAKE_PIN=1 run pair "$KEYS" && rc=0 || rc=$?
[ "$rc" = 0 ] || fail "pair asked for a PIN exited $rc"
grep -qx 0000 "$TMP/agent.log" || fail "the PIN asked for was not 0000"

# a device omega-zero still offers, which bluetoothd forgot after it stopped looking
reset
up
reach "$KEYS" 'Keychron K3' input-keyboard no no no
run pair "$KEYS" && rc=0 || rc=$?
[ "$rc" = 0 ] || fail "pairing with a device bluetoothd forgot exited $rc"
[ "$(grep -x -e 'scan on' -e 'scan off' -e "pair $KEYS" "$TMP/agent.log" | tr '\n' ' ')" \
	= "scan on scan off pair $KEYS " ] \
	|| fail "the device was not looked for again before the pairing: $(tr '\n' ' ' < "$TMP/agent.log")"
grep -qx '	Paired: yes' "$TMP/info/$KEYS" || fail "the device found again did not pair"

reset
up
known "$KEYS" 'Keychron K3' input-keyboard no no no
start="$(date +%s)"
FAKE_PAIR_FAIL=1 run pair "$KEYS" && rc=0 || rc=$?
[ "$rc" = 3 ] || fail "a refused pairing exited $rc"
[ $(($(date +%s) - start)) -lt 2 ] || fail "a refused pairing waited out its time"
grep -qx "trust $KEYS" "$TMP/agent.log" && fail "a device that did not pair was trusted"
run pair 00:11:22:33:44:55 && rc=0 || rc=$?
[ "$rc" = 3 ] || fail "pairing with a device not in reach exited $rc"
FAKE_CONNECT_FAIL=1 run pair "$KEYS" && rc=0 || rc=$?
[ "$rc" = 4 ] || fail "paired but not connected exited $rc"
grep -qx '	Paired: yes' "$TMP/info/$KEYS" || fail "a pairing whose connection failed was lost"

reset
up
known "$CANS" 'WH-1000XM4' audio-headphones yes no no
run connect "$CANS" && rc=0 || rc=$?
[ "$rc" = 0 ] || fail "connect exited $rc"
grep -qx "trust $CANS" "$TMP/bluetoothctl.log" || fail "connect did not trust the device first"
grep -qx "connect $CANS" "$TMP/bluetoothctl.log" || fail "connect did not connect"
grep -qx '	Connected: yes' "$TMP/info/$CANS" || fail "the device is not connected"
FAKE_CONNECT_FAIL=1 run connect "$CANS" && rc=0 || rc=$?
[ "$rc" = 4 ] || fail "a failed connection exited $rc"
run disconnect "$CANS"
grep -qx '	Connected: no' "$TMP/info/$CANS" || fail "disconnect left the device connected"
run forget "$CANS"
[ -e "$TMP/info/$CANS" ] && fail "forget left the device known"
grep -q "$CANS" "$TMP/devices" && fail "forget left the device listed"

for verb in pair connect disconnect forget; do
	run "$verb" 'nope' && rc=0 || rc=$?
	[ "$rc" = 2 ] || fail "$verb with an address that is none exited $rc"
	run "$verb" '8C:EA:48:12:34:5G' && rc=0 || rc=$?
	[ "$rc" = 2 ] || fail "$verb with a letter past F exited $rc"
	run "$verb" && rc=0 || rc=$?
	[ "$rc" = 2 ] || fail "$verb with no address exited $rc"
done

reset
up
run scan on
settle there "$TMP/discovering"
attach="$(cat "$TMP/hciattach.pid")"
run down && rc=0 || rc=$?
[ "$rc" = 0 ] || fail "down exited $rc"
[ -f "$TMP/run/omega-bt.up" ] && fail "down left the mark"
grep -qx 'stop bluetooth' "$TMP/systemctl.log" || fail "down left bluetoothd running"
grep -qx 'hci0 down' "$TMP/hciconfig.log" || fail "down did not take hci0 down"
kill -0 "$attach" 2>/dev/null && fail "down left rtk_hciattach running"
[ -e "$TMP/run/omega-bt-attach.pid" ] && fail "down left rtk_hciattach's pid"
settle gone "$TMP/discovering" || fail "down left discovery running"
[ "$(cat "$TMP/rfkill/rfkill1/soft")" = 1 ] || fail "down left the radio unblocked"
[ "$(cat "$TMP/rfkill/rfkill0/soft")" = 0 ] || fail "down blocked Wi-Fi"
run status | grep -qx enabled=no || fail "status after down still says enabled"

reset
up
attach="$(cat "$TMP/hciattach.pid")"
rm -f "$TMP/run/omega-bt-attach.pid"
run down
kill -0 "$attach" 2>/dev/null && fail "down left an rtk_hciattach it had not started running"

run bogus 2>/dev/null && rc=0 || rc=$?
[ "$rc" = 2 ] || fail "an unknown verb exited $rc"

if [ "$FAILS" -gt 0 ]; then
	echo "bluetooth: $FAILS failed"
	exit 1
fi
echo "bluetooth: all passed"
