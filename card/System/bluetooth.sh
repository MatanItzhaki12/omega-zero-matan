#!/bin/sh
# Bluetooth. BaseOS boots with the radio blocked and nothing driving it: up unblocks it, loads
# the Realtek chip's firmware over its UART with rtk_hciattach, and starts bluetoothd; down
# undoes all of it. Pairings are bluetoothd's own, kept in /data/bluetooth. status prints the
# devices the radio knows one to a line, a tab between each field, for omega-zero to read:
#
#   device=<MAC>	<audio|gamepad|keyboard|other>	<paired>	<trusted>	<connected>	<name>
#
# paired, trusted and connected are yes or no. The name is the one the device gave, else the alias
# it was given, unless that is only its address, as bluetoothd aliases a device that gave no
# name: a device only found with neither is left out, as most are beacons nobody can pair with.
# One paired is always listed, by its alias whatever that is.
#
# Exit codes: 0 done, 1 Bluetooth is not up (or would not come up), 2 a verb or address this
# script does not take, 3 the pairing failed, 4 paired but the connection failed.
set -u

RUN="${AGS_RUN:-/run}"
RFKILL="${AGS_RFKILL_SYS:-/sys/class/rfkill}"
BT_SYS="${AGS_BT_SYS:-/sys/class/bluetooth}"
TTY="${AGS_BT_TTY:-/dev/ttyS1}"
BAUD="${AGS_BT_BAUD:-115200}"
WAIT_S="${AGS_BT_WAIT_S:-10}"
SCAN_S="${AGS_BT_SCAN_S:-20}"
FIND_S="${AGS_BT_FIND_S:-10}"
PAIR_S="${AGS_BT_PAIR_S:-20}"
CONNECT_S="${AGS_BT_CONNECT_S:-15}"
CTL_S="${AGS_BT_CTL_S:-5}"

HCIATTACH="${AGS_RTK_HCIATTACH:-rtk_hciattach}"
HCICONFIG="${AGS_HCICONFIG:-hciconfig}"
BLUETOOTHCTL="${AGS_BLUETOOTHCTL:-bluetoothctl}"
SYSTEMCTL="${AGS_SYSTEMCTL:-systemctl}"
TIMEOUT="${AGS_TIMEOUT:-timeout}"
PIDOF="${AGS_PIDOF:-pidof}"
BLUEALSA="${AGS_BLUEALSA:-bluealsa}"

HCI=hci0
UP_MARK="$RUN/omega-bt.up"
ATTACH_PID="$RUN/omega-bt-attach.pid"
SCAN_PID="$RUN/omega-bt-scan.pid"
AUDIO_PID="$RUN/omega-bt-audio.pid"
PAIR_FIFO="$RUN/omega-bt-pair.fifo"
PAIR_LOG="$RUN/omega-bt-pair.log"

nap() {
	sleep 0.25 2>/dev/null || sleep 1
}

alive() {
	[ -s "$1" ] && kill -0 "$(cat "$1")" 2>/dev/null
}

# Stops the process $1 holds the pid of, and waits a moment for it to be gone: a bluetoothctl
# ends its discovery as it exits.
stop_pid() {
	if alive "$1"; then
		pid="$(cat "$1")"
		kill "$pid" 2>/dev/null
		w=0
		while kill -0 "$pid" 2>/dev/null && [ "$w" -lt 8 ]; do
			nap
			w=$((w + 1))
		done
	fi
	/bin/rm -f "$1"
}

# bluetoothctl waits for bluetoothd for as long as it takes, so none is ever run without a limit.
ctl() {
	$TIMEOUT "$CTL_S" $BLUETOOTHCTL "$@" </dev/null 2>/dev/null
}

running() {
	$SYSTEMCTL is-active bluetooth >/dev/null 2>&1
}

# A Bluetooth address: six pairs of hex digits with colons between.
mac_ok() {
	case "$1" in
	??:??:??:??:??:??) ;;
	*) return 1 ;;
	esac
	case "$1" in
	*[!0-9A-Fa-f:]*) return 1 ;;
	esac
}

# An address written with dashes or colons, as bluetoothd aliases a device that gave no name.
address() {
	mac_ok "$(printf '%s' "$1" | tr - :)"
}

# The radio's rfkill switch: the one whose type is bluetooth.
switch() {
	for r in "$RFKILL"/rfkill*; do
		if [ "$(cat "$r/type" 2>/dev/null)" = bluetooth ]; then
			echo "$r"
			return 0
		fi
	done
	return 1
}

block() {
	s="$(switch)" || return 0
	echo "$1" > "$s/soft" 2>/dev/null
}

wait_hci() {
	w=0
	while [ ! -d "$BT_SYS/$HCI" ] && [ "$w" -lt $(($1 * 4)) ]; do
		nap
		w=$((w + 1))
	done
	[ -d "$BT_SYS/$HCI" ]
}

# $1 of the controller's `show`: yes or no.
shown() {
	ctl show | sed -n "s/^[[:space:]]*$1: //p" | head -n 1
}

wait_powered() {
	w=0
	while [ "$(shown Powered)" != yes ] && [ "$w" -lt $(($1 * 4)) ]; do
		nap
		w=$((w + 1))
	done
	[ "$(shown Powered)" = yes ]
}

# rtk_hciattach loads the firmware and must stay running to keep the UART a Bluetooth line, so
# it runs in the foreground of a process of its own.
attach() {
	$HCIATTACH -n -s "$BAUD" "$TTY" rtk_h5 </dev/null >/dev/null 2>&1 &
	echo $! > "$ATTACH_PID"
}

# bluez-alsa: the A2DP source headphones connect to, and the ALSA device omega-zero plays
# into. Without it bluetoothd has no audio profile to give headphones, which then connect and
# drop at once. It must be up before they connect, so it starts with bluetoothd.
audio_up() {
	command -v "$BLUEALSA" >/dev/null 2>&1 || return 0
	alive "$AUDIO_PID" && return 0
	"$BLUEALSA" -p a2dp-source --keep-alive=5 </dev/null >/dev/null 2>&1 &
	echo $! > "$AUDIO_PID"
}

bt_up() {
	: > "$UP_MARK"
	block 0
	if [ ! -d "$BT_SYS/$HCI" ]; then
		alive "$ATTACH_PID" || attach
		wait_hci "$WAIT_S" || return 1
	fi
	$HCICONFIG "$HCI" up >/dev/null 2>&1 || return 1
	$SYSTEMCTL start bluetooth >/dev/null 2>&1 || return 1
	audio_up
	# main.conf's AutoEnable powers the controller once bluetoothd finds it; ask, if it has not
	wait_powered 2 && return 0
	ctl power on >/dev/null
	wait_powered "$WAIT_S"
}

bt_down() {
	/bin/rm -f "$UP_MARK"
	stop_pid "$SCAN_PID"
	stop_pid "$AUDIO_PID"
	$SYSTEMCTL stop bluetooth >/dev/null 2>&1
	if [ -d "$BT_SYS/$HCI" ]; then
		$HCICONFIG "$HCI" down >/dev/null 2>&1
	fi
	stop_pid "$ATTACH_PID"
	# one started by hand, outside omega-zero, would keep the radio too
	name="$(basename "$HCIATTACH")"
	pids="$($PIDOF "$name" 2>/dev/null)"
	if [ -n "$pids" ]; then
		kill $pids 2>/dev/null
	fi
	# it lets the UART go as it exits, which takes it a moment
	w=0
	while $PIDOF "$name" >/dev/null 2>&1 && [ "$w" -lt 8 ]; do
		nap
		w=$((w + 1))
	done
	block 1
}

# $2 of a device's `info` text $1.
field() {
	printf '%s\n' "$1" | sed -n "s/^[[:space:]]*$2: //p" | head -n 1
}

yes_no() {
	if [ "$1" = yes ]; then echo yes; else echo no; fi
}

bt_devices() {
	ctl devices | while read -r word mac _; do
		[ "$word" = Device ] && mac_ok "$mac" || continue
		info="$(ctl info "$mac")" || continue
		paired="$(yes_no "$(field "$info" Paired)")"
		name="$(field "$info" Name)"
		if [ -z "$name" ]; then
			alias="$(field "$info" Alias)"
			if [ "$paired" = yes ] || ! address "$alias"; then
				name="$alias"
			fi
		fi
		[ -n "$name" ] || [ "$paired" = yes ] || continue
		case "$(field "$info" Icon)" in
		audio-card | audio-headset | audio-headphones) kind=audio ;;
		input-gaming) kind=gamepad ;;
		input-keyboard) kind=keyboard ;;
		*) kind=other ;;
		esac
		printf 'device=%s\t%s\t%s\t%s\t%s\t%s\n' "$mac" "$kind" "$paired" \
			"$(yes_no "$(field "$info" Trusted)")" "$(yes_no "$(field "$info" Connected)")" "$name"
	done
}

bt_status() {
	if [ -f "$UP_MARK" ]; then echo enabled=yes; else echo enabled=no; fi
	if ! running; then
		echo powered=no
		echo discovering=no
		return 0
	fi
	show="$(ctl show)"
	echo "powered=$(yes_no "$(field "$show" Powered)")"
	echo "discovering=$(yes_no "$(field "$show" Discovering)")"
	bt_devices
}

# Discovery lasts as long as the bluetoothctl that asked for it: until omega-zero turns it off,
# which it does once it has looked for long enough, and at most SCAN_S, so a frontend that is
# gone stops it too.
bt_scan() {
	case "${1:-}" in
	on)
		running || return 1
		alive "$SCAN_PID" && return 0
		$BLUETOOTHCTL --timeout "$SCAN_S" scan on </dev/null >/dev/null 2>&1 &
		echo $! > "$SCAN_PID"
		;;
	off) stop_pid "$SCAN_PID" ;;
	*) return 2 ;;
	esac
}

# bluetoothd knows device $1: it is paired, or was found not long ago.
known() {
	ctl info "$1" | grep -qi "^Device $1 ("
}

# A line for the pairing bluetoothctl to run.
say() {
	echo "$*" >&3
}

# How many times the pairing bluetoothctl has printed $1.
said() {
	grep -o "$1" "$PAIR_LOG" 2>/dev/null | wc -l
}

# Waits up to $2 seconds for the pairing bluetoothctl to print $1.
logged() {
	w=0
	while ! grep -q "$1" "$PAIR_LOG" 2>/dev/null && [ "$w" -lt $(($2 * 4)) ]; do
		nap
		w=$((w + 1))
	done
	grep -q "$1" "$PAIR_LOG" 2>/dev/null
}

# Waits up to $3 seconds for device $1's $2 to be yes, giving up early once the pairing
# bluetoothctl has printed something $4 (an extended regular expression) matches. Whatever its
# agent is asked meanwhile it says yes to, and a PIN an older device asks for is 0000, as most
# of those take.
until_yes() {
	w=0
	while [ "$w" -lt $(($3 * 4)) ]; do
		if ctl info "$1" | grep -q "^[[:space:]]*$2: yes"; then
			return 0
		fi
		if [ -n "$4" ] && grep -E -q "$4" "$PAIR_LOG" 2>/dev/null; then
			return 1
		fi
		asked="$(said '(yes/no)')"
		while [ "$answered" -lt "$asked" ]; do
			say yes
			answered=$((answered + 1))
		done
		pins="$(said 'Enter PIN code')"
		while [ "$pinned" -lt "$pins" ]; do
			say 0000
			pinned=$((pinned + 1))
		done
		nap
		w=$((w + 1))
	done
	return 1
}

# bluetoothd forgets a device only found within half a minute of discovery stopping, while
# omega-zero still offers it: the pairing bluetoothctl looks for $1 again, for up to $2 seconds,
# until it is back.
refind() {
	known "$1" && return 0
	say "scan on"
	w=0
	while ! known "$1" && [ "$w" -lt $(($2 * 4)) ]; do
		nap
		w=$((w + 1))
	done
	say "scan off"
}

# Pairs with $1, trusts it so it may connect again by itself, then connects it. One bluetoothctl
# stays up throughout as the agent, NoInputNoOutput: the pairing is Just Works, which most
# gamepads and headphones take.
bt_pair() {
	mac_ok "${1:-}" || return 2
	running || return 1
	# discovery slows a pairing down; omega-zero looks again only when asked
	stop_pid "$SCAN_PID"
	/bin/rm -f "$PAIR_FIFO" "$PAIR_LOG"
	mkfifo "$PAIR_FIFO" || return 1
	$BLUETOOTHCTL --agent NoInputNoOutput <"$PAIR_FIFO" >"$PAIR_LOG" 2>&1 &
	agent=$!
	# read and write, so the open never waits on a bluetoothctl that did not start
	exec 3<>"$PAIR_FIFO"
	answered=0
	pinned=0
	rc=3
	if logged 'Agent registered' "$CTL_S"; then
		# the default agent is the one asked about a device connecting back as it pairs
		say default-agent
		logged 'Default agent request successful' "$CTL_S"
		refind "$1" "$FIND_S"
		say "pair $1"
		if until_yes "$1" Paired "$PAIR_S" 'Failed to pair|not available'; then
			say "trust $1"
			until_yes "$1" Trusted "$CTL_S" ''
			say "connect $1"
			rc=4
			if until_yes "$1" Connected "$CONNECT_S" 'Failed to connect'; then
				rc=0
			fi
		fi
	fi
	say quit
	exec 3>&-
	w=0
	while kill -0 "$agent" 2>/dev/null && [ "$w" -lt 8 ]; do
		nap
		w=$((w + 1))
	done
	kill "$agent" 2>/dev/null
	/bin/rm -f "$PAIR_FIFO" "$PAIR_LOG"
	return "$rc"
}

# A device paired before is trusted first, so it may connect again by itself after.
bt_connect() {
	mac_ok "${1:-}" || return 2
	running || return 1
	ctl trust "$1" >/dev/null
	$TIMEOUT "$CONNECT_S" $BLUETOOTHCTL connect "$1" </dev/null >/dev/null 2>&1 || return 4
}

bt_disconnect() {
	mac_ok "${1:-}" || return 2
	running || return 1
	ctl disconnect "$1" >/dev/null || return 1
}

bt_forget() {
	mac_ok "${1:-}" || return 2
	running || return 1
	ctl remove "$1" >/dev/null || return 1
}

case "${1:-}" in
up) bt_up ;;
down) bt_down ;;
status) bt_status ;;
scan) bt_scan "${2:-}" ;;
pair) bt_pair "${2:-}" ;;
connect) bt_connect "${2:-}" ;;
disconnect) bt_disconnect "${2:-}" ;;
forget) bt_forget "${2:-}" ;;
*)
	echo "usage: bluetooth.sh up|down|status|scan on|off|pair MAC|connect MAC|disconnect MAC|forget MAC" >&2
	exit 2
	;;
esac
