/*
  What each of flow.html's screens does with a button: where its cursor goes, what its values
  are, and which screen a row opens. player.js does the moving between them, and opens the power
  menu on any screen when POWER is held; this file only answers each press with nothing,
  { go: <screen> } to go deeper or { back: true }. A screen whose bar hints follow its cursor or
  its state (Network's X SCAN with Wi-Fi on, Y INFO on NETWORK's rows and Y FORGET on the network
  joined, Bluetooth's X SCAN while the radio is on and Y FORGET on a paired device) says them with
  hint().

  The settings that show on more than one screen (Wi-Fi, Network Time, SSH, Network's scan,
  Bluetooth and its devices, the date, the backdrop, the bar's font, the shelf's system and the
  letter jumped to, which the bar's pill shows) live once, in FlowScreens.model, and every screen
  showing one follows it.
*/
window.FlowScreens = (() => {
  // ---- the settings, shared ----

  // The Bluetooth devices the flow knows of, by address, one of each kind: a gamepad connected
  // and headphones paired, which the screen lists from the start, and a keyboard and a band in
  // reach, which a look finds, the keyboard first. A device found that gives no name is never
  // listed, so every one here has one. A device is found, pairing, paired, connecting or
  // connected; its kind is the icon its row shows.
  const DEVICES = [
    { mac: 'E4:17:D8:2C:41:07', name: '8BitDo SN30 Pro', kind: 'gamepad', state: 'connected' },
    { mac: '38:18:4C:B2:9A:10', name: 'WH-1000XM4', kind: 'audio', state: 'paired' },
    { mac: 'DC:2C:26:F1:05:3B', name: 'Keychron K3', kind: 'keyboard', state: 'found' },
    { mac: 'C8:0F:10:4D:7A:25', name: 'Mi Smart Band 4', kind: 'other', state: 'found' },
  ];
  const PAIRED = DEVICES.filter(device => device.state !== 'found');

  const model = {
    wifi: true,
    ntp: true,            // the clock set from the network
    ssh: true,            // the SSH server running
    network: 'Home',      // the network joined
    joining: 'Home',      // the one the keyboard asks the passphrase of
    found: false,         // Network shows what a scan found since it opened
    scanning: false,      // X's scan under way
    bluetooth: true,      // the Bluetooth radio on
    devices: PAIRED,      // what the screen lists, top to bottom, never changed in place
    look: 'never',        // the look for devices: never since the screen opened, looking, found or nothing
    date: { year: 2026, month: 10, day: 10, hour: 16, minute: 0, zone: 0 },
    style: 'grid',
    moving: true,
    colour: 'night',
    font: 'bold',         // the bottom bar's labels, bold or thin
    system: 'GBA',        // the shelf on show, its system in the bar's pill
    letter: null,         // the letter Up or Down jumped to, in the pill instead for a while
  };
  const listeners = new Set();
  function set(key, value) {
    if (model[key] === value) return;
    model[key] = value;
    for (const listener of listeners) listener(key, value);
  }
  const onChange = listener => listeners.add(listener);

  // The colours whose names are two words, as the Theme's Color row writes them.
  const COLOUR_NAMES = { firered: 'FIRE RED', leafgreen: 'LEAF GREEN' };

  // How a row with data-set shows a setting, and reads it back. A switch is ON or OFF.
  const SWITCH = { show: on => (on ? 'ON' : 'OFF'), read: value => value === 'ON' };
  const CODECS = {
    wifi: SWITCH,
    ntp: SWITCH,
    ssh: SWITCH,
    bluetooth: SWITCH,
    moving: { show: on => (on ? 'MOVING' : 'STILL'), read: value => value === 'MOVING' },
    style: { show: name => name.toUpperCase(), read: value => value.toLowerCase() },
    colour: { show: name => COLOUR_NAMES[name] ?? name.toUpperCase(), read: value => value.toLowerCase().replace(' ', '') },
    font: { show: name => name.toUpperCase(), read: value => value.toLowerCase() },
  };

  const MONTHS = ['JAN', 'FEB', 'MAR', 'APR', 'MAY', 'JUN', 'JUL', 'AUG', 'SEP', 'OCT', 'NOV', 'DEC'];
  const two = n => String(n).padStart(2, '0');
  const daysIn = (year, month) => new Date(year, month, 0).getDate();
  // a zone as Date & Time shows it: UTC+00:00
  const zoneOf = zone => `UTC${zone < 0 ? '-' : '+'}${two(Math.abs(zone))}:00`;

  // ---- a menu: rows, top to bottom across its panels, one under the cursor ----

  class Menu {
    constructor(page) {
      this.page = page;
      this.rows = [...page.querySelectorAll('omega-row, omega-list-row')];
      this.panels = [...page.querySelectorAll('omega-panel')];
      this.cursor = 0;
      this.light();
    }

    get row() { return this.rows[this.cursor]; }

    // the cursor's row lit, and the panel holding it active
    light() {
      this.rows.forEach((row, i) => row.toggleAttribute('lit', i === this.cursor));
      for (const panel of this.panels) panel.toggleAttribute('active', panel.contains(this.row));
      this.reveal();
    }

    // In a list taller than its panel, scroll it a whole row at a time to keep the cursor in it.
    reveal() {
      const list = this.row.closest('.list');
      if (!list) return;
      const top = this.row.offsetTop;
      const bottom = top + this.row.offsetHeight;
      if (top < list.scrollTop) list.scrollTop = top;
      else if (bottom > list.scrollTop + list.clientHeight) list.scrollTop = bottom - list.clientHeight;
    }

    move(by) {
      this.cursor = Math.max(0, Math.min(this.rows.length - 1, this.cursor + by));
      this.light();
    }

    // ◀ ▶ step a row's value through its data-values, round and round
    change(by) {
      const values = this.row.dataset.values?.split('|');
      if (!values) return;
      const at = values.indexOf(this.row.getAttribute('value'));
      const value = values[(at + by + values.length) % values.length];
      this.row.setAttribute('value', value);
      const key = this.row.dataset.set;
      if (key) set(key, CODECS[key].read(value));
    }

    // the cursor onto the row that opens `screen`, as it was when that screen was opened: with
    // A, or with Y on the row whose panel it is
    focus(screen) {
      const at = this.rows.findIndex(row => (row.dataset.opens === screen || row.dataset.info === screen) &&
        (row.localName !== 'omega-list-row' || row.getAttribute('label') === model.joining));
      if (at < 0) return;
      this.cursor = at;
      this.light();
    }

    press(key) {
      switch (key) {
        case 'up': return this.move(-1);
        case 'down': return this.move(1);
        case 'left': return this.change(-1);
        case 'right': return this.change(1);
        case 'a': return this.confirm();
        case 'y': return this.info();
        case 'b': return { back: true };
      }
    }

    // A on a setting that is a switch, ON or OFF, turns it the other way, as ◀ ▶ do; on any
    // other row it opens the row's screen
    confirm() {
      if (this.row.dataset.set && this.row.dataset.values === 'ON|OFF') return this.change(1);
      return this.open();
    }

    open() {
      const screen = this.row.dataset.opens;
      if (!screen || this.row.hasAttribute('dim')) return;
      if (this.row.localName === 'omega-list-row') set('joining', this.row.getAttribute('label'));
      return { go: screen };
    }

    // Y on a row with a panel of its own (data-info) opens it
    info() {
      const screen = this.row.dataset.info;
      if (screen) return { go: screen };
    }
  }

  // Settings: START goes back to the shelf too. Network opens without what an earlier visit's
  // scan found.
  class Settings extends Menu {
    press(key) {
      return key === 'start' ? { back: true } : super.press(key);
    }

    open() {
      const answer = super.open();
      if (answer?.go === 'network') set('found', false);
      return answer;
    }
  }

  // A screen with nothing to pick: B goes back.
  class Still {
    constructor(page) { this.page = page; }
    press(key) { if (key === 'b') return { back: true }; }
  }

  // The screen after the power menu's choice, shown until the handheld goes down: nothing can be
  // pressed.
  class Going {
    constructor(page) { this.page = page; }
    press() {}
  }

  // The shelves, one a system, L1 and R1 going round them; and the letters the placeholder carts
  // start with, the same on every shelf, which Up and Down jump between.
  const SYSTEMS = ['GBA', 'GB', 'GBC', 'SNES', 'PS1'];
  const INITIALS = ['A', 'C', 'F', 'K', 'M', 'P', 'S', 'Z'];

  // The shelf, as far as these screens go: START to Settings, SELECT to Controls, L1 and R1 to
  // the shelf before or after, round and round, and Up and Down to the letter before or after,
  // stopping at the first and the last. Each shelf keeps the letter it is on. What the bar's
  // pill shows of it, and for how long, is player.js's.
  class Shelf extends Still {
    constructor(page) {
      super(page);
      this.at = Object.fromEntries(SYSTEMS.map(system => [system, 0]));
    }

    press(key) {
      if (key === 'start') return { go: 'settings' };
      if (key === 'select') return { go: 'controls' };
      if (key === 'l1' || key === 'r1') {
        const at = SYSTEMS.indexOf(model.system) + (key === 'r1' ? 1 : -1);
        set('letter', null);
        set('system', SYSTEMS[(at + SYSTEMS.length) % SYSTEMS.length]);
      }
      if (key === 'up' || key === 'down') {
        const at = this.at[model.system] + (key === 'down' ? 1 : -1);
        if (at < 0 || at >= INITIALS.length) return;
        this.at[model.system] = at;
        set('letter', INITIALS[at]);
      }
    }
  }
  let shelf;

  // The shelf on `system`, the letter `letter` (or none) shown in the pill and its carts on it.
  function shelfAs(system, letter) {
    set('system', system);
    if (letter) shelf.at[system] = INITIALS.indexOf(letter);
    set('letter', letter);
  }

  // ---- Date & Time: a line of fields, ◀ ▶ between them, ▲ ▼ to change one ----

  const FIELDS = ['year', 'month', 'day', 'hour', 'minute', 'zone'];
  class Picker {
    constructor(page) {
      this.page = page;
      this.line = page.querySelector('.picker');
      this.cursor = 0;
      this.draw();
    }

    // 2026 - 10 - 10   16 : 00   UTC+00:00, each field its own box so the cursor can underline it
    draw() {
      const { year, month, day, hour, minute, zone } = model.date;
      const text = {
        year: String(year), month: two(month), day: two(day), hour: two(hour), minute: two(minute),
        zone: zoneOf(zone),
      };
      const parts = [['year'], ' - ', ['month'], ' - ', ['day'], '   ', ['hour'], ' : ', ['minute'], '   ', ['zone']];
      this.line.replaceChildren(...parts.map(part => {
        const span = document.createElement('span');
        if (typeof part === 'string') span.textContent = part;
        else {
          span.className = 'field';
          span.textContent = text[part[0]];
          span.classList.toggle('lit', FIELDS[this.cursor] === part[0]);
        }
        return span;
      }));
      this.line.style.setProperty('--chars', this.line.textContent.length);
    }

    step(by) {
      const date = { ...model.date };
      const field = FIELDS[this.cursor];
      const wrap = (value, lo, hi) => lo + ((((value - lo) % (hi - lo + 1)) + (hi - lo + 1)) % (hi - lo + 1));
      if (field === 'year') date.year = wrap(date.year + by, 2000, 2099);
      if (field === 'month') date.month = wrap(date.month + by, 1, 12);
      if (field === 'day') date.day = wrap(date.day + by, 1, daysIn(date.year, date.month));
      if (field === 'hour') date.hour = wrap(date.hour + by, 0, 23);
      if (field === 'minute') date.minute = wrap(date.minute + by, 0, 59);
      if (field === 'zone') date.zone = wrap(date.zone + by, -12, 14);
      date.day = Math.min(date.day, daysIn(date.year, date.month));
      set('date', date);
    }

    focus() {}

    press(key) {
      switch (key) {
        case 'left': this.cursor = Math.max(0, this.cursor - 1); return this.draw();
        case 'right': this.cursor = Math.min(FIELDS.length - 1, this.cursor + 1); return this.draw();
        case 'up': return this.step(1);
        case 'down': return this.step(-1);
        case 'b': return { back: true };
      }
    }
  }

  // ---- the keyboard: a field, 10 by 4 keys and a row of wide ones ----

  // Sizes in art pixels (see flow.css): a key, the room between keys.
  const KEY_W = 23;
  const KEY_H = 19;
  const GAP = 3;
  const LETTERS = ['1234567890', 'qwertyuiop', 'asdfghjkl-', 'zxcvbnm_.@'];
  const SYMBOLS = ['!?#$%&*()=', '+/\\|[]{}<>', '\'":;,^~`£¢', '§°¡¿«»¤¥©®'];
  // the bottom row, each key as many columns wide as it says
  const WIDE = [['SHIFT', 2], ['#+=', 1], ['SPACE', 3], ['DELETE', 2], ['DONE', 2]];

  class Keyboard {
    constructor(page) {
      this.page = page;
      this.panel = page.querySelector('omega-panel');
      this.grid = page.querySelector('.keys');
      this.typed = page.querySelector('.typed');
      this.text = '';
      this.shift = false;
      this.symbols = false;
      this.at = { row: 0, col: 0 };
      this.draw();
    }

    // every key's label, row by row: 4 rows of ten single keys, then the wide ones
    layout() {
      const rows = (this.symbols ? SYMBOLS : LETTERS).map(row => [...row].map(ch => [this.shift ? ch.toUpperCase() : ch, 1]));
      return [...rows, WIDE.map(([label, span]) => [label === '#+=' && this.symbols ? 'abc' : label, span])];
    }

    // The key under the cursor: on the bottom row, the wide key over the remembered column.
    key() {
      const row = this.layout()[this.at.row];
      let col = 0;
      for (let i = 0; i < row.length; i++) {
        col += row[i][1];
        if (this.at.col < col) return { row: this.at.row, index: i, label: row[i][0] };
      }
      return { row: this.at.row, index: row.length - 1, label: row.at(-1)[0] };
    }

    // Each key placed in whole art pixels, its label centred by its ink: n letters are 6n - 1
    // pixels of ink, and a key's width is odd, so the room either side is the same.
    draw() {
      const u = n => `calc(${n} * var(--u))`;
      const lit = this.key();
      this.grid.replaceChildren(...this.layout().flatMap((row, y) => {
        let col = 0;
        return row.map(([label, span], i) => {
          const key = document.createElement('div');
          const width = span * KEY_W + (span - 1) * GAP;
          key.className = 'key';
          key.classList.toggle('lit', y === lit.row && i === lit.index);
          key.textContent = label;
          key.style.left = u(col * (KEY_W + GAP));
          key.style.top = u(y * (KEY_H + GAP));
          key.style.width = u(width);
          key.style.paddingLeft = u((width - 2 - (6 * label.length - 1)) / 2);
          col += span;
          return key;
        });
      }));
      // the field shows the end of what is typed when it is longer than the field
      this.typed.textContent = this.text.slice(-40);
    }

    move(dx, dy) {
      const rows = this.layout();
      if (dy) this.at.row = Math.max(0, Math.min(rows.length - 1, this.at.row + dy));
      if (dx) {
        // step a whole key: from the key's first column, past its width
        const row = rows[this.at.row];
        const { index } = this.key();
        const next = Math.max(0, Math.min(row.length - 1, index + dx));
        this.at.col = row.slice(0, next).reduce((sum, [, span]) => sum + span, 0);
      }
      this.draw();
    }

    type(label) {
      if (label === 'SHIFT') this.shift = !this.shift;
      else if (label === '#+=' || label === 'abc') this.symbols = !this.symbols;
      else if (label === 'SPACE') this.text += ' ';
      else if (label === 'DELETE') this.text = this.text.slice(0, -1);
      else if (label === 'DONE') return this.done();
      else this.text += label;
      this.draw();
    }

    // joined: back to the networks, which now show this one connected
    done() {
      set('network', model.joining);
      return { back: true };
    }

    focus() {}

    press(key) {
      switch (key) {
        case 'up': return this.move(0, -1);
        case 'down': return this.move(0, 1);
        case 'left': return this.move(-1, 0);
        case 'right': return this.move(1, 0);
        case 'a': return this.type(this.key().label);
        case 'start': return this.done();
        // B deletes, as the hint line says, and goes back once there is nothing left to delete
        case 'b':
          if (!this.text) return { back: true };
          this.text = this.text.slice(0, -1);
          return this.draw();
      }
    }
  }

  // ---- Network: NETWORK's rows, then the networks known, and those X's scan finds ----

  // The networks the flow's scan finds, strongest first: each one's bars lit, and its padlock.
  const NETWORKS = [
    { name: 'Home', signal: 3, lock: true },
    { name: 'Café Wi-Fi', signal: 2, lock: false },
    { name: 'Upstairs 5G', signal: 1, lock: true },
  ];
  // How long the flow's scan takes, in ms.
  const SCAN_MS = 1200;
  let scanTimer = 0;

  class Network extends Menu {
    constructor(page) {
      super(page);
      this.settings = [...this.rows];
      this.list = page.querySelector('.list');
      this.note = this.list.querySelector('.note');
      this.draw();
    }

    // The networks' rows drawn again from the model, the cursor staying on the row it was on (on
    // the one now in its place when that one is gone), and the note under them. Before a scan
    // the list holds the network joined alone, which the handheld has saved, with no bars or
    // padlock, which only a scan hears; once one is done, every network it found, kept while
    // the next looks. With Wi-Fi off there are none.
    draw() {
      const lit = this.row;
      const scanned = model.found;
      const shown = !model.wifi ? [] : scanned ? NETWORKS : [{ name: model.network }];
      const kept = new Map([...this.list.querySelectorAll('omega-list-row')].map(row => [row.dataset.network, row]));
      const rows = shown.map(network => {
        const row = kept.get(network.name) ?? document.createElement('omega-list-row');
        kept.delete(network.name);
        row.dataset.network = network.name;
        row.dataset.opens = 'keyboard';
        row.setAttribute('label', network.name);
        if (network.name === model.network) row.setAttribute('value', 'CONNECTED');
        else row.removeAttribute('value');
        if (scanned) row.setAttribute('signal', network.signal);
        else row.removeAttribute('signal');
        row.toggleAttribute('lock', scanned && network.lock);
        return row;
      });
      for (const row of kept.values()) row.remove();
      if (rows.some((row, i) => this.list.children[i] !== row)) this.note.before(...rows);
      note(this.note, !model.wifi ? 'Wi-Fi is off.' :
        model.scanning ? 'Looking for networks…' :
        !model.found ? 'Press X to look for networks.' : '');
      this.rows = [...this.settings, ...rows];
      const at = this.rows.indexOf(lit);
      this.cursor = at >= 0 ? at : Math.min(this.cursor, this.rows.length - 1);
      this.light();
    }

    // X SCAN in the bar while Wi-Fi is on, and beside it Y INFO on NETWORK's rows, which says so
    // with Wi-Fi off too, or Y FORGET on the network joined.
    hint() {
      if (this.row.dataset.info) return model.wifi ? 'X SCAN | Y INFO' : 'Y INFO';
      if (!model.wifi) return null;
      return this.row.dataset.network === model.network ? 'X SCAN | Y FORGET' : 'X SCAN';
    }

    // X looks for networks once, while Wi-Fi is on and no scan is under way.
    press(key) {
      if (key === 'x') return this.scan();
      return super.press(key);
    }

    scan() {
      if (!model.wifi || model.scanning) return;
      set('scanning', true);
      clearTimeout(scanTimer);
      scanTimer = setTimeout(() => {
        set('found', true);
        set('scanning', false);
      }, SCAN_MS);
    }
  }

  // Network's scan as the URL's scan= has it: none since the screen opened, running (the first
  // scan looking, standing still for a shot) or done. Wi-Fi off has none.
  const SCAN = ['none', 'running', 'done'];
  function scanAs(state) {
    clearTimeout(scanTimer);
    set('found', model.wifi && state === 'done');
    set('scanning', model.wifi && state === 'running');
  }
  // Which of those Network's scan is in now, as near as it comes: a scan again over what one
  // found is running.
  function scanState() {
    return model.scanning ? 'running' : model.found ? 'done' : 'none';
  }

  // ---- Bluetooth: the radio's row, then while it is on the devices paired and those X finds ----

  // How long the flow takes to pair with a device, and to connect to a paired one, in ms.
  const PAIRING_MS = 1500;
  const CONNECTING_MS = 1000;
  // How long a look for devices lasts, shorter than the handheld's 12 s, and when it finds each
  // device in reach, in ms.
  const LOOK_MS = 6000;
  const FINDS_MS = [1500, 3500];
  // What the note under the devices says of the look, while the radio is on.
  const LOOK_NOTE = {
    never: 'Press X to look for devices.',
    looking: 'Looking for devices…',
    found: '',
    nothing: 'No devices found.',
  };

  // What a device's row says it is to the handheld; nothing for one only found.
  const DEVICE_VALUE = {
    connected: 'CONNECTED', connecting: 'CONNECTING', pairing: 'PAIRING', paired: 'PAIRED', found: '',
  };
  // A device the handheld is paired with, which Y forgets.
  const isPaired = device => ['connected', 'connecting', 'paired'].includes(device.state);
  // A device's name as its row shows it: its own, or the address of a paired one that gives none.
  const nameOf = device => device.name ?? device.mac;

  // `mac`'s device in `state`, the rest as they are.
  function setDevice(mac, state) {
    set('devices', model.devices.map(device => (device.mac === mac ? { ...device, state } : device)));
  }

  // A note's text, any … in it drawn as flow/art.js's glyph (the web font has none); hidden when
  // there is nothing to say.
  function note(p, text) {
    p.replaceChildren(...text.split('…').flatMap((part, i) => {
      if (!i) return [part];
      const glyph = document.createElement('i');
      glyph.className = 'g ellipsis';
      return [glyph, part];
    }));
    p.hidden = !text;
  }

  class Bluetooth extends Menu {
    constructor(page) {
      super(page);
      this.radio = this.rows[0];
      this.list = page.querySelector('.devices');
      this.note = this.list.querySelector('.note');
      this.timers = [];
      this.found = 0;   // the devices the look under way, or the last, has found
      this.draw();
    }

    // The device under the cursor, when it is on one.
    device() {
      return model.devices.find(device => device.mac === this.row.dataset.device);
    }

    // The devices' rows drawn again from the model, the cursor staying on the row it was on (on
    // the one now in its place when that one is gone), and the note under them: the radio is
    // off, or what the look has come to.
    draw() {
      const lit = this.row;
      const kept = new Map([...this.list.querySelectorAll('omega-list-row')].map(row => [row.dataset.device, row]));
      const rows = model.devices.map(device => {
        const row = kept.get(device.mac) ?? document.createElement('omega-list-row');
        kept.delete(device.mac);
        row.dataset.device = device.mac;
        row.setAttribute('label', nameOf(device));
        row.setAttribute('device', device.kind);
        if (DEVICE_VALUE[device.state]) row.setAttribute('value', DEVICE_VALUE[device.state]);
        else row.removeAttribute('value');
        row.hidden = !model.bluetooth;
        return row;
      });
      for (const row of kept.values()) row.remove();
      if (rows.some((row, i) => this.list.children[i] !== row)) this.note.before(...rows);
      note(this.note, model.bluetooth ? LOOK_NOTE[model.look] : 'Bluetooth is off.');
      this.rows = [this.radio, ...(model.bluetooth ? rows : [])];
      const at = this.rows.indexOf(lit);
      this.cursor = at >= 0 ? at : Math.min(this.cursor, this.rows.length - 1);
      this.light();
    }

    // The cursor on `mac`'s device, or on the radio's row.
    home(mac) {
      this.cursor = Math.max(0, this.rows.findIndex(row => row.dataset.device === mac));
      this.light();
    }

    // X SCAN in the bar while the radio is on, and beside it Y FORGET while the cursor is on a
    // paired device.
    hint() {
      if (!model.bluetooth) return null;
      const device = this.device();
      return device && isPaired(device) ? 'X SCAN | Y FORGET' : 'X SCAN';
    }

    press(key) {
      const device = this.device();
      if (device && key === 'a') return this.pick(device);
      if (device && key === 'y' && isPaired(device)) return this.forget(device);
      if (key === 'x') return this.look();
      if (key === 'b') this.unlook();
      return super.press(key);
    }

    // X: one look for devices, which finds those in reach and not listed one after another, then
    // stops by itself. Nothing while one is under way, with the radio off, or while a device
    // pairs or connects. What an earlier look found goes, to be found again.
    look() {
      const busy = model.devices.some(device => ['pairing', 'connecting'].includes(device.state));
      if (!model.bluetooth || model.look === 'looking' || busy) return;
      set('devices', model.devices.filter(device => device.state !== 'found'));
      set('look', 'looking');
      this.found = 0;
      const reach = DEVICES.filter(device => device.state === 'found' &&
        !model.devices.some(known => known.mac === device.mac));
      reach.forEach((device, i) => this.later(FINDS_MS[i] ?? FINDS_MS.at(-1), () => {
        this.found += 1;
        set('devices', [...model.devices, device]);
      }));
      this.later(LOOK_MS, () => this.stop());
    }

    later(ms, then) {
      this.timers.push(setTimeout(then, ms));
    }

    // The timers of a look dropped, whatever it has come to.
    halt() {
      for (const timer of this.timers) clearTimeout(timer);
      this.timers = [];
    }

    // The look over: it has looked long enough, or a pairing stops it.
    stop() {
      this.halt();
      if (model.look === 'looking') set('look', this.found ? 'found' : 'nothing');
    }

    // The look stopped and what it found let go, as the screen is left or the radio goes off.
    unlook() {
      this.halt();
      set('devices', model.devices.filter(device => device.state !== 'found'));
      set('look', 'never');
    }

    // A on a device: one only found pairs and then connects, which ends a look under way, a
    // paired one connects, a connected one disconnects. One already on its way is left to get
    // there.
    pick(device) {
      if (device.state === 'found') {
        this.stop();
        this.towards(device, 'pairing', PAIRING_MS);
      } else if (device.state === 'paired') this.towards(device, 'connecting', CONNECTING_MS);
      else if (device.state === 'connected') setDevice(device.mac, 'paired');
    }

    // `device` `state` for `ms`, then connected, unless something else has happened to it since.
    towards(device, state, ms) {
      setDevice(device.mac, state);
      setTimeout(() => {
        if (model.devices.find(d => d.mac === device.mac)?.state === state) setDevice(device.mac, 'connected');
      }, ms);
    }

    // Y on a paired device: forgotten, it leaves the list, and the cursor takes the row after it.
    forget(device) {
      set('devices', model.devices.filter(d => d.mac !== device.mac));
    }
  }

  // Bluetooth as the URL's bt= has it, the cursor on the radio's row: on as the screen opens,
  // before any look, with the paired devices; off; on with nothing paired, before any look; a
  // look under way, held, the keyboard found so far; after it, the band found too; or after it
  // with the keyboard pairing and the cursor on it.
  const BT = ['on', 'off', 'empty', 'looking', 'found', 'pairing'];
  let bluetoothMenu = null;
  function bluetoothAs(state) {
    const [keyboard, band] = DEVICES.filter(device => device.state === 'found');
    bluetoothMenu?.halt();
    set('bluetooth', state !== 'off');
    set('devices', {
      empty: [],
      looking: [...PAIRED, keyboard],
      found: [...PAIRED, keyboard, band],
      pairing: [...PAIRED, { ...keyboard, state: 'pairing' }, band],
    }[state] ?? PAIRED);
    set('look', { looking: 'looking', found: 'found', pairing: 'found' }[state] ?? 'never');
    if (bluetoothMenu) bluetoothMenu.found = state === 'looking' ? 1 : 2;
    bluetoothMenu?.home(state === 'pairing' ? keyboard.mac : null);
  }
  // Which of those Bluetooth is in now, as near as it comes.
  function bluetoothState() {
    if (!model.bluetooth) return 'off';
    if (model.devices.some(device => device.state === 'pairing')) return 'pairing';
    if (model.look === 'looking') return 'looking';
    if (model.look !== 'never') return 'found';
    return model.devices.length ? 'on' : 'empty';
  }

  // ---- every screen, and keeping them in step with the settings ----

  function build(screen) {
    const page = name => screen.querySelector(`[data-screen="${name}"]`);
    const screens = {
      shelf: new Shelf(page('shelf')),
      settings: new Settings(page('settings')),
      controls: new Still(page('controls')),
      network: new Network(page('network')),
      wifi: new Still(page('wifi')),
      time: new Still(page('time')),
      ssh: new Still(page('ssh')),
      keyboard: new Keyboard(page('keyboard')),
      bluetooth: new Bluetooth(page('bluetooth')),
      labels: new Menu(page('labels')),
      datetime: new Picker(page('datetime')),
      background: new Menu(page('background')),
      about: new Still(page('about')),
      power: new Menu(page('power')),
      restarting: new Going(page('restarting')),
      poweroff: new Going(page('poweroff')),
    };

    // what shows a setting, drawn again from the model
    // The panels Y opens on Network's rows, and the value of a row of one, by its label.
    const panel = name => page(name).querySelector('.info');
    const fill = (name, label, value) =>
      panel(name).querySelector(`omega-row[label="${label}"]`).setAttribute('value', value);

    function show() {
      const { date, wifi, ntp, ssh } = model;
      for (const row of screen.querySelectorAll('[data-set]')) {
        row.setAttribute('value', CODECS[row.dataset.set].show(model[row.dataset.set]));
      }
      screen.querySelector('[data-show="date"]').setAttribute('value',
        `${MONTHS[date.month - 1]} ${date.day} ${two(date.hour)}:${two(date.minute)}`);
      screen.querySelector('[data-show="network"]').setAttribute('value', wifi ? model.network.toUpperCase() : 'OFF');
      screens.network.draw();
      // each panel its rows, or, offline, what to do first: WI-FI's needs Wi-Fi on, SSH's SSH
      // and Wi-Fi, NETWORK TIME's only itself, and without Wi-Fi it waits for one
      panel('wifi').classList.toggle('offline', !wifi);
      fill('wifi', 'NETWORK', model.network);
      panel('time').classList.toggle('offline', !ntp);
      fill('time', 'STATUS', wifi ? 'SYNCED' : 'WAITING');
      fill('time', 'DATE', `${date.year}-${two(date.month)}-${two(date.day)}`);
      fill('time', 'TIME', `${two(date.hour)}:${two(date.minute)}`);
      fill('time', 'ZONE', zoneOf(date.zone));
      panel('time').querySelector('.when-online .note').textContent =
        wifi ? 'Set the zone in Date & Time' : 'Join a Wi-Fi network first.';
      panel('ssh').classList.toggle('offline', !ssh || !wifi);
      panel('ssh').querySelector('.when-offline .note').textContent =
        ssh ? 'Join a Wi-Fi network first.' : 'Turn SSH on first.';
      page('keyboard').querySelector('omega-panel').setAttribute('label', `PASSPHRASE FOR ${model.joining.toUpperCase()}`);
      // Settings' BLUETOOTH: the device connected, else whether the radio is on
      const connected = model.devices.find(device => device.state === 'connected');
      screen.querySelector('[data-show="bluetooth"]').setAttribute('value',
        !model.bluetooth ? 'OFF' : connected ? nameOf(connected).toUpperCase() : 'ON');
      screens.bluetooth.draw();
      screens.datetime.draw();
    }
    show();
    onChange(show);
    // Wi-Fi off forgets what a scan found, and stops one under way; the Bluetooth radio going
    // off ends a look, and lets go of what it found
    onChange((key, on) => {
      if (key === 'wifi' && !on) scanAs('none');
      if (key === 'bluetooth' && !on) screens.bluetooth.unlook();
    });
    bluetoothMenu = screens.bluetooth;
    shelf = screens.shelf;
    return screens;
  }

  // Each screen's parent, the one B goes back to. The power menu's is the screen POWER was held
  // on, which player.js puts here as it opens it: the shelf until then.
  const PARENT = {
    settings: 'shelf', controls: 'shelf',
    network: 'settings', bluetooth: 'settings', labels: 'settings', datetime: 'settings',
    background: 'settings', about: 'settings',
    wifi: 'network', time: 'network', ssh: 'network', keyboard: 'network',
    power: 'shelf', restarting: 'power', poweroff: 'power',
  };

  // The screens after the power menu's choice, which have no bar: nothing can be pressed on them.
  const GOING = ['restarting', 'poweroff'];

  return {
    model, set, onChange, build, PARENT, GOING, SCAN, scanAs, scanState, BT, bluetoothAs, bluetoothState,
    SYSTEMS, INITIALS, shelfAs,
  };
})();
