//! Link play from the game's menu: the menu SELECT + MENU opens, picking how to link, the
//! rooms online and the fields typed for them, and starting, watching and ending a link.

use super::*;

impl App {
    pub(super) fn open_game_menu(&mut self) {
        if self.game_menu.is_some() {
            return;
        }
        if self.link_active() {
            return self.refuse();
        }
        // link play is the GBA's and the Game Boys': a SNES or PlayStation game never starts it
        if !matches!(self.platform, Platform::Gba | Platform::Gb | Platform::Gbc) {
            self.hud.toast(Toast::NoLink, self.now());
            return;
        }
        if self.core == Core::Mgba {
            let wireless = self
                .seated()
                .is_some_and(|stem| self.link_mode(stem).0 == LinkKind::Wireless);
            if wireless {
                self.hud.toast(Toast::NeedsGpsp, self.now());
                return;
            }
            self.link_hardware = LinkKind::Cable;
            self.radio.ask(RadioJob::Warm);
            self.game_menu = Some(GameMenu::Pick(self.last_role));
            return;
        }
        if self.platform != Platform::Gba {
            self.hud.toast(Toast::NoLink, self.now());
            return;
        }
        let carried = self
            .seated()
            .and_then(|stem| self.auto_link(stem))
            .is_some_and(|(cart, _)| link_carried(&cart.code, &cart.title));
        if !carried {
            self.hud.toast(Toast::NoLink, self.now());
            return;
        }
        if self.core != Core::Gpsp {
            self.hud.toast(Toast::NeedsGpsp, self.now());
            return;
        }
        let hardware = self
            .seated()
            .map_or(LinkKind::Cable, |stem| self.link_mode(stem).0);
        self.link_hardware = hardware;
        self.radio.ask(RadioJob::Warm);
        self.game_menu = Some(GameMenu::Pick(self.last_role));
    }

    /// SELECT + MENU in a game: the DISC screen on a PlayStation game, else the link screen, or
    /// the link under way.
    pub(super) fn game_menu_shortcut(&mut self) {
        if self.swaps_discs() {
            return self.open_disc_screen();
        }
        let Some(client_id) = self.link_client_id() else {
            return self.open_game_menu();
        };
        let now = self.now();
        self.game_menu = Some(GameMenu::Linked {
            role: LinkRow::from_client_id(client_id),
            worked: now,
            since: now,
            opened: true,
        });
    }

    pub(super) fn end_link_from_menu(&mut self) {
        let role = self
            .link_client_id()
            .map_or(self.last_role, LinkRow::from_client_id);
        self.end_link();
        self.hud.toast(Toast::LinkEnded, self.now());
        self.unplug(role);
    }

    pub(super) fn unplug(&mut self, role: LinkRow) {
        self.game_menu = Some(GameMenu::Unplug {
            role,
            since: self.now(),
        });
    }

    pub fn set_radio_jobs(&mut self, jobs: Box<dyn RadioJobs>) {
        self.radio = jobs;
    }

    pub(super) fn game_menu_input(&mut self, action: Action) {
        let Some(menu) = self.game_menu else {
            return;
        };
        match menu {
            // the keyboard over the link screen, for a row or a room's password
            GameMenu::Pick(_) | GameMenu::Rooms { .. } if self.link_typing.is_some() => {
                self.link_typing_input(action);
            }
            GameMenu::Pick(role) => {
                use crate::link_screen::LinkField;
                let field = self.lit_field();
                match action {
                    Action::GbaDown(Btn::Up) => self.move_link_field(false),
                    Action::GbaDown(Btn::Down) => self.move_link_field(true),
                    Action::GbaDown(b @ (Btn::Left | Btn::Right)) => match field {
                        LinkField::Play => self.set_link_online(!self.going_online()),
                        LinkField::With => self.set_link_retroarch(!self.link_retroarch),
                        LinkField::Role => {
                            self.last_role = role.other();
                            self.game_menu = Some(GameMenu::Pick(role.other()));
                        }
                        LinkField::Mode => self.switch_hardware(),
                        LinkField::Server => {
                            let n = crate::netplay_lobby::RELAYS.len() as u8;
                            let r = self.state.relay;
                            self.state.relay = match b == Btn::Right {
                                true => (r + 1) % n,
                                false => (r + n - 1) % n,
                            };
                            self.persist();
                        }
                        LinkField::Advanced => self.link_advanced = !self.link_advanced,
                        LinkField::Name
                        | LinkField::Password
                        | LinkField::Address
                        | LinkField::RoomPassword => {}
                    },
                    Action::GbaDown(Btn::A) if field.typed() => self.open_link_typing(field),
                    // the rows' own keys before they were rows: SELECT for MODE, Y for online
                    Action::GbaDown(Btn::Select) if self.via_netplay() => self.refuse(),
                    Action::GbaDown(Btn::Select) => self.switch_hardware(),
                    Action::GbaDown(Btn::Y) if self.online_possible() => {
                        self.set_link_online(!self.going_online());
                    }
                    Action::GbaDown(Btn::Y) => self.refuse(),
                    Action::GbaDown(Btn::A) if self.via_netplay() => self.pick_netplay(role),
                    Action::GbaDown(Btn::A) => self.pick_link(role),
                    Action::GbaDown(Btn::B) | Action::GameMenu => self.close_game_menu(),
                    _ => {}
                }
            }
            GameMenu::Rooms { lit } => match action {
                Action::GbaDown(Btn::Up) => {
                    self.game_menu = Some(GameMenu::Rooms {
                        lit: lit.saturating_sub(1),
                    });
                }
                Action::GbaDown(Btn::Down) => {
                    let last = self.online_rooms.len().saturating_sub(1);
                    self.game_menu = Some(GameMenu::Rooms {
                        lit: (lit + 1).min(last),
                    });
                }
                Action::GbaDown(Btn::Y) => self.list_rooms(),
                Action::GbaDown(Btn::A) => match self.online_rooms.get(lit).cloned() {
                    // a room with a password asks for it first, as RetroArch's lobby does
                    Some(room) if room.has_password => self.ask_room_password(room, false),
                    Some(room) => {
                        self.join_password.clear();
                        self.join_room(room);
                    }
                    None => self.refuse(),
                },
                Action::GbaDown(Btn::B) => self.game_menu = Some(GameMenu::Pick(LinkRow::Join)),
                Action::GameMenu => self.close_game_menu(),
                _ => {}
            },
            GameMenu::Working { .. } => {
                if action == Action::GbaDown(Btn::B) {
                    if let Some(starting) = &mut self.starting {
                        starting.starter.cancel();
                    }
                    if let Some(reload) = &mut self.reload {
                        reload.cancelled = true;
                    }
                }
            }
            GameMenu::Linked { opened: true, .. } => match action {
                Action::GbaDown(Btn::A) => self.end_link_from_menu(),
                Action::GbaDown(Btn::B) | Action::GameMenu => self.close_game_menu(),
                _ => {}
            },
            GameMenu::Linked { .. } => {}
            GameMenu::Failed { .. } => {
                if matches!(action, Action::GbaDown(Btn::A | Btn::B) | Action::GameMenu) {
                    self.close_game_menu();
                }
            }
            GameMenu::Unplug { .. } => {}
        }
    }

    pub fn start_link(&mut self, starter: LinkStarter, client_id: u16) {
        self.start_link_from(starter, client_id, self.now());
    }

    pub(super) fn start_link_from(&mut self, starter: LinkStarter, client_id: u16, since: Millis) {
        if let Some(mut old) = self.starting.replace(LinkStarting { starter, client_id }) {
            old.starter.cancel();
        }
        self.game_menu = Some(GameMenu::Working {
            role: LinkRow::from_client_id(client_id),
            step: LinkStep::Radio,
            since,
        });
    }

    /// Whether the game in play can link online: a GBA game on gpSP, over the cable, whose
    /// link packets RetroArch's netplay carries. The wireless adapter's are left nearby, as
    /// GB-Link leaves them: a relay's delay is more than they take.
    pub(crate) fn online_available(&self) -> bool {
        self.platform == Platform::Gba
            && self.core == Core::Gpsp
            && self.link_hardware == LinkKind::Cable
    }

    /// Whether the seated game could link online, were it on the cable: the PLAY row is shown.
    pub(crate) fn online_possible(&self) -> bool {
        self.platform == Platform::Gba
            && self.core == Core::Gpsp
            && (self.link_hardware == LinkKind::Cable
                || self.seated().is_some_and(|stem| self.link_switchable(stem)))
    }

    /// The link screen is set to go online.
    pub(crate) fn going_online(&self) -> bool {
        self.online_available() && self.link_online
    }

    /// The link screen is set to link with RetroArch on this network: local, with its netplay.
    pub(crate) fn going_lan(&self) -> bool {
        self.online_available() && !self.link_online && self.link_retroarch
    }

    /// The link goes through RetroArch's netplay: online, or on this network.
    pub(crate) fn via_netplay(&self) -> bool {
        self.going_online() || self.going_lan()
    }

    /// PLAY set to Online or Local. Online is the cable's: the adapter's games go back to it.
    pub(super) fn set_link_online(&mut self, online: bool) {
        if online && self.link_hardware != LinkKind::Cable {
            self.switch_hardware();
        }
        self.link_online = online && self.online_available();
    }

    /// WITH set to RetroArch or omega-zero. RetroArch's netplay carries the cable alone, as
    /// online.
    pub(super) fn set_link_retroarch(&mut self, retroarch: bool) {
        if retroarch && self.link_hardware != LinkKind::Cable {
            self.switch_hardware();
        }
        self.link_retroarch = retroarch && self.online_available();
    }

    /// Up or down the LINK panel's rows, stopping at either end as a settings list does.
    pub(super) fn move_link_field(&mut self, down: bool) {
        let fields = self.link_fields();
        let at = fields
            .iter()
            .position(|f| *f == self.lit_field())
            .unwrap_or(0);
        let to = match down {
            true => (at + 1).min(fields.len().saturating_sub(1)),
            false => at.saturating_sub(1),
        };
        if let Some(f) = fields.get(to) {
            self.link_field = *f;
        }
    }

    /// The keyboard up for a row, holding what is there now.
    pub(super) fn open_link_typing(&mut self, field: crate::link_screen::LinkField) {
        use crate::link_screen::LinkField;
        let text = match field {
            LinkField::Name => self.online_name(),
            LinkField::Address => self.state.online_address.clone(),
            LinkField::RoomPassword => String::new(),
            _ => self.state.online_password.clone(),
        };
        self.link_refused = false;
        self.link_typing = Some((
            field,
            omega_ui::Keyboard {
                text,
                ..omega_ui::Keyboard::default()
            },
        ));
    }

    /// The keyboard up for the password of `room`, which the link then joins; `refused` when
    /// the one given before was turned away.
    pub(super) fn ask_room_password(&mut self, room: crate::netplay_lobby::Room, refused: bool) {
        self.pending_join = Some(room);
        self.open_link_typing(crate::link_screen::LinkField::RoomPassword);
        self.link_refused = refused;
    }

    /// The link to `room` started, with the password given for it.
    pub(super) fn join_room(&mut self, room: crate::netplay_lobby::Room) {
        self.pending_join = Some(room.clone());
        self.online_room = Some(room);
        self.pick_link(LinkRow::Join);
    }

    /// The keys while the link screen's keyboard is up. What is typed is kept on START or the
    /// keyboard's enter, even nothing: no name is the default one, and no password is none.
    pub(super) fn link_typing_input(&mut self, action: Action) {
        use crate::link_screen::LinkField;
        let Some((field, keys)) = &mut self.link_typing else {
            return;
        };
        let field = *field;
        let mut submit = false;
        match action {
            Action::GbaDown(Btn::Up) => keys.up(),
            Action::GbaDown(Btn::Down) => keys.down(),
            Action::GbaDown(Btn::Left) => keys.left(),
            Action::GbaDown(Btn::Right) => keys.right(),
            Action::GbaDown(Btn::A) => submit = keys.press(),
            Action::GbaDown(Btn::B) if keys.text.is_empty() => self.link_typing = None,
            Action::GbaDown(Btn::B) => keys.delete(),
            Action::GbaDown(Btn::X) => keys.shift(),
            Action::GbaDown(Btn::Y) => keys.type_char(' '),
            Action::GbaDown(Btn::L1 | Btn::R1) => keys.symbols(),
            Action::GbaDown(Btn::Start) => submit = true,
            Action::GameMenu | Action::QuickMenu => self.link_typing = None,
            _ => {}
        }
        if !submit {
            return;
        }
        let Some((_, keys)) = self.link_typing.take() else {
            return;
        };
        match field {
            LinkField::Name => self.state.online_name = omega_store::online_name(&keys.text),
            LinkField::Address => {
                self.state.online_address = omega_store::online_password(&keys.text);
            }
            LinkField::RoomPassword => {
                self.join_password = keys.text;
                if let Some(room) = self.pending_join.clone() {
                    self.join_room(room);
                }
                return;
            }
            _ => self.state.online_password = omega_store::online_password(&keys.text),
        }
        self.persist();
    }

    /// A on a link through RetroArch's netplay: hosting opens a room straight away; joining
    /// goes to the address typed, under ADVANCED online, or lists the rooms open first.
    pub(super) fn pick_netplay(&mut self, role: LinkRow) {
        if !self.online() {
            let now = self.now();
            self.game_menu = Some(GameMenu::Failed {
                role,
                fail: LinkFail::NoWifi,
                worked: now,
                since: now,
            });
            return;
        }
        match role {
            LinkRow::Host => {
                self.online_room = None;
                self.pick_link(role);
            }
            LinkRow::Join => match self.typed_address() {
                Some(room) => {
                    self.join_password.clear();
                    self.join_room(room);
                }
                None => self.list_rooms(),
            },
        }
    }

    /// The room at the address typed under ADVANCED, while it is shown: a host, and its port
    /// after a colon, RetroArch's own when none is given.
    pub(super) fn typed_address(&self) -> Option<crate::netplay_lobby::Room> {
        let typed = self.state.online_address.trim();
        if !(self.going_online() && self.link_advanced) || typed.is_empty() {
            return None;
        }
        let (host, port) = match typed.rsplit_once(':') {
            Some((host, port)) if !host.contains(':') => (host, port.parse().ok()?),
            _ => (typed, crate::netplay_lobby::NETPLAY_PORT),
        };
        Some(crate::netplay_lobby::Room {
            username: typed.to_string(),
            game_name: String::new(),
            game_crc: 0,
            core_name: crate::netplay::CORE_NAME.to_string(),
            ip: host.to_string(),
            port,
            has_password: false,
            mitm_ip: String::new(),
            mitm_port: 0,
            mitm_session: String::new(),
            connectable: true,
            country: String::new(),
        })
    }

    /// The ROOMS panel holding `rooms`, as though they had been listed: for tests, which reach
    /// no lobby.
    #[doc(hidden)]
    pub fn show_rooms(&mut self, rooms: Vec<crate::netplay_lobby::Room>) {
        self.rooms_fetch = None;
        self.rooms_failed = false;
        self.online_rooms = rooms;
        self.game_menu = Some(GameMenu::Rooms { lit: 0 });
    }

    /// The ROOMS panel, the lobby or this network asked for the rooms open.
    pub(super) fn list_rooms(&mut self) {
        self.online_rooms.clear();
        self.rooms_failed = false;
        self.rooms_fetch = Some(match self.going_lan() {
            true => crate::link_start::RoomsFetch::spawn_lan(),
            false => crate::link_start::RoomsFetch::spawn(crate::netplay_lobby::LOBBY),
        });
        self.game_menu = Some(GameMenu::Rooms { lit: 0 });
    }

    /// The rooms, once they have been listed: those playing the game in play first, then those
    /// with no password.
    pub(super) fn poll_rooms(&mut self) {
        let Some(fetch) = &self.rooms_fetch else {
            return;
        };
        if let Some(rooms) = fetch.poll() {
            self.rooms_fetch = None;
            match rooms {
                Ok(mut rooms) => {
                    let ours = self
                        .seated()
                        .map(crate::link_screen::short_game)
                        .unwrap_or_default();
                    rooms.sort_by_key(|r| {
                        (
                            crate::link_screen::short_game(&r.game_name) != ours,
                            r.has_password,
                        )
                    });
                    self.online_rooms = rooms;
                }
                Err(()) => self.rooms_failed = true,
            }
        }
    }

    /// The ROOMS panel's note: the rooms being looked for, why none is listed, or that the lit
    /// one has a password.
    pub(crate) fn rooms_status(&self, lit: usize) -> Option<&'static str> {
        let lan = self.going_lan();
        if self.rooms_fetch.is_some() {
            Some(match lan {
                true => "Looking on this Wi-Fi",
                false => "Finding rooms",
            })
        } else if self.rooms_failed {
            Some(match lan {
                true => "Could not look on this Wi-Fi",
                false => LinkFail::Lobby.line(),
            })
        } else if self.online_rooms.is_empty() {
            Some(match lan {
                true => "No rooms. Relay must be off",
                false => "No rooms open right now",
            })
        } else if self.online_rooms.get(lit).is_some_and(|r| r.has_password) {
            Some("Has a password: A asks for it")
        } else {
            None
        }
    }

    /// What starts a link as `role`: through RetroArch's netplay, online or on this network,
    /// when the link screen says so; else nearby, over the handhelds' own radio.
    pub(super) fn link_starter(&self, role: LinkRow) -> LinkStarter {
        use crate::link_start::OnlineJob;
        if !self.via_netplay() {
            return LinkStarter::spawn(role.role(), link_port());
        }
        let nick = self.online_name();
        let password = self.state.online_password.clone();
        let cart = self.seated_cart();
        let game = cart
            .map(|c| c.stem.clone())
            .or_else(|| self.seated().map(str::to_string))
            .unwrap_or_default();
        let rom = cart.map(|c| c.rom.clone());
        let job = match (role, self.online_room.clone()) {
            (LinkRow::Join, Some(room)) => OnlineJob::Join {
                nick,
                room,
                password: self.join_password.clone(),
            },
            _ if self.going_lan() => OnlineJob::HostLan {
                nick,
                game,
                crc: 0,
                rom,
                password,
                port: crate::netplay_lobby::NETPLAY_PORT,
                discovery: crate::netplay_lan::DISCOVERY_PORT,
            },
            _ => OnlineJob::Host {
                nick,
                relay: crate::netplay_lobby::RELAYS
                    .get(self.state.relay as usize)
                    .unwrap_or(&crate::netplay_lobby::RELAYS[1])
                    .to_string(),
                game,
                crc: 0,
                rom,
                password,
            },
        };
        LinkStarter::spawn_online(job, crate::netplay_lobby::LOBBY)
    }

    pub(super) fn switch_hardware(&mut self) {
        let Some(stem) = self.seated().map(str::to_string) else {
            return;
        };
        if !self.link_switchable(&stem) {
            return self.refuse();
        }
        let other = self.link_hardware.other();
        self.link_hardware = other;
        self.link_choices.insert(stem, other);
    }

    pub(crate) fn link_switchable(&self, stem: &str) -> bool {
        self.auto_link(stem).is_some_and(|(cart, auto)| {
            serial_option(self.link_hardware.other(), auto, &cart.code, &cart.title)
                != serial_option(self.link_hardware, auto, &cart.code, &cart.title)
        })
    }

    pub(super) fn pick_link(&mut self, role: LinkRow) {
        let Some(stem) = self.seated().map(str::to_string) else {
            return;
        };
        let loaded = self.link_loaded.unwrap_or("auto");
        if self.core == Core::Mgba {
            let player = role.client_id() as u8;
            if self.link_player == Some(player) {
                return self.start_link(
                    LinkStarter::spawn(role.role(), link_port()),
                    role.client_id(),
                );
            }
            if self.snapshot.as_ref().is_some_and(|s| !s.resume_trusted()) {
                return self.refuse();
            }
            self.link_player = Some(player);
            self.link_reload = Some((stem.clone(), loaded));
            self.reload = Some(Reload {
                stem,
                role,
                cancelled: false,
                from: self.link_hardware,
                from_serial: loaded,
                fallback: false,
            });
            self.game_menu = Some(GameMenu::Working {
                role,
                step: LinkStep::Radio,
                since: self.now(),
            });
            return;
        }
        let (_, serial) = self.link_mode(&stem);
        if serial == loaded {
            return self.start_link(self.link_starter(role), role.client_id());
        }
        if self.snapshot.as_ref().is_some_and(|s| !s.resume_trusted()) {
            return self.refuse();
        }
        self.link_reload = Some((stem.clone(), serial));
        self.reload = Some(Reload {
            stem,
            role,
            cancelled: false,
            from: self.link_hardware.other(),
            from_serial: loaded,
            fallback: false,
        });
        self.game_menu = Some(GameMenu::Working {
            role,
            step: LinkStep::Radio,
            since: self.now(),
        });
    }

    pub(super) fn refuse_seated(&mut self) {
        self.close_game_menu();
        let caught = self.seat();
        let cart = match &mut self.phase {
            Phase::Playing { cart } => Some(std::mem::take(cart)),
            Phase::Doze { cart } => {
                *cart = None;
                None
            }
            _ => None,
        };
        if let Some(cart) = cart {
            self.refuse_out(cart, caught);
        }
    }

    pub(super) fn close_game_menu(&mut self) {
        self.game_menu = None;
        self.link_typing = None;
        self.link_field = crate::link_screen::LinkField::Role;
        self.disc_screen = None;
        if let Some(mut starting) = self.starting.take() {
            starting.starter.cancel();
        }
        if !self.link_active() {
            self.radio.ask(RadioJob::Cool);
        }
        let uncollected =
            self.link_reload.is_some() && self.reload.as_ref().is_some_and(|r| !r.fallback);
        if uncollected {
            self.link_reload = None;
            self.reload = None;
        } else if let Some(reload) = &mut self.reload {
            reload.cancelled = true;
        }
    }

    pub(super) fn working_role(&self, client_id: u16) -> (LinkRow, Millis) {
        match self.game_menu {
            Some(GameMenu::Working { role, since, .. }) => (role, since),
            _ => (LinkRow::from_client_id(client_id), self.now()),
        }
    }

    pub(super) fn poll_link(&mut self) {
        if self.power_screen.is_some() {
            return;
        }
        let Some(mut starting) = self.starting.take() else {
            return;
        };
        match starting.starter.poll() {
            None => self.starting = Some(starting),
            Some(LinkProgress::At(step)) => {
                if let Some(GameMenu::Working { role, since, .. }) = self.game_menu {
                    self.game_menu = Some(GameMenu::Working { role, step, since });
                }
                self.starting = Some(starting);
            }
            Some(LinkProgress::Ready(ready)) => {
                let (role, worked) = self.working_role(starting.client_id);
                self.game_menu = Some(GameMenu::Linked {
                    role,
                    worked,
                    since: self.now(),
                    opened: false,
                });
                // online, the client the core plays as is the one the room numbered it
                let client_id = ready.client.unwrap_or(starting.client_id);
                self.link_peer = ready.peer;
                self.begin_link(client_id);
                self.link_transport = Some((client_id, ready.link));
            }
            Some(LinkProgress::Failed(LinkFail::Cancelled)) => self.game_menu = None,
            Some(LinkProgress::Failed(LinkFail::Password))
                if starting.client_id == LinkRow::Join.client_id()
                    && self.pending_join.is_some() =>
            {
                let room = self.pending_join.clone().expect("the room");
                self.game_menu = Some(GameMenu::Pick(LinkRow::Join));
                self.ask_room_password(room, true);
            }
            Some(LinkProgress::Failed(fail)) => {
                let (role, worked) = self.working_role(starting.client_id);
                self.game_menu = Some(GameMenu::Failed {
                    role,
                    fail,
                    worked,
                    since: self.now(),
                });
            }
        }
    }
}
