//! With SCRATCH_DIR set, writes the core picker's faces there as PNGs to look at: each at the
//! size it is shown (2x its art) and at four times that, and the board put together with its
//! sockets and chip, seated, mid-hop, and in a red and a grey shell; and the SNES board in the
//! American cart's shell and the Super Famicom's.

use omega_store::{Cart, Core, Platform};
use omega_ui::{
    board_face, chip_face, chip_shadow_face, lid_shadow_face, socket_face, CartFace, CHIP_U,
    CHIP_V, HOP_LIFT, SHADOW_AT, SOCKET_U, SOCKET_V,
};

fn write(path: &std::path::Path, f: &CartFace, zoom: u32) {
    let (zw, zh) = (f.w * zoom, f.h * zoom);
    let mut px = vec![0u8; (zw * zh * 4) as usize];
    for y in 0..zh {
        for x in 0..zw {
            let s = (((y / zoom) * f.w + x / zoom) * 4) as usize;
            let d = ((y * zw + x) * 4) as usize;
            px[d..d + 4].copy_from_slice(&f.rgba[s..s + 4]);
        }
    }
    let file = std::fs::File::create(path).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), zw, zh);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(&px).unwrap();
}

fn over(dst: &mut CartFace, src: &CartFace, x: i32, y: i32) {
    for row in 0..src.h as i32 {
        for col in 0..src.w as i32 {
            let s = ((row * src.w as i32 + col) * 4) as usize;
            let (dx, dy) = (x + col, y + row);
            let inside = (0..dst.w as i32).contains(&dx) && (0..dst.h as i32).contains(&dy);
            if src.rgba[s + 3] > 0 && inside {
                let d = ((dy * dst.w as i32 + dx) * 4) as usize;
                dst.rgba[d..d + 4].copy_from_slice(&src.rgba[s..s + 4]);
            }
        }
    }
}

#[test]
fn render_board() {
    let Ok(dir) = std::env::var("SCRATCH_DIR") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let both = |name: &str, f: &CartFace| {
        write(&dir.join(format!("{name}-1x.png")), f, 2);
        write(&dir.join(format!("{name}-4x.png")), f, 8);
    };
    let cart = Cart {
        platform: Platform::Gba,
        stem: "Pokemon - Emerald Version (USA, Europe)".into(),
        rom: "Games/GBA/Emerald.gba".into(),
        label: None,
        code: "BPEE".into(),
        shell: None,
        title: "POKEMON EMER".into(),
    };
    let board = board_face(&cart);
    both("board", &board);
    for core in Core::ALL {
        both(&format!("chip-{}", core.text()), &chip_face(Some(core)));
        both(&format!("socket-{}", core.text()), &socket_face(core));
    }
    both("chip-blank", &chip_face(None));
    both("shadow", &chip_shadow_face());
    both("lid-shadow", &lid_shadow_face());

    let socketed = |board: &CartFace| {
        let mut f = CartFace {
            rgba: board.rgba.clone(),
            w: board.w,
            h: board.h,
        };
        for (i, core) in Core::for_platform(Platform::Gba).iter().enumerate() {
            over(
                &mut f,
                &socket_face(*core),
                SOCKET_U[i] as i32,
                SOCKET_V as i32,
            );
        }
        f
    };
    let mut seated = socketed(&board);
    over(
        &mut seated,
        &chip_face(Some(Core::Mgba)),
        CHIP_U[0] as i32,
        CHIP_V as i32,
    );
    both("board-seated", &seated);

    let mut hop = socketed(&board);
    let u = f32::midpoint(CHIP_U[0], CHIP_U[1]);
    let (sx, sy) = (u + SHADOW_AT[0], CHIP_V + SHADOW_AT[1]);
    over(&mut hop, &chip_shadow_face(), sx as i32, sy as i32);
    over(
        &mut hop,
        &chip_face(None),
        u as i32,
        (CHIP_V - HOP_LIFT) as i32,
    );
    both("board-hop", &hop);

    // the SNES board, its two wider sockets Snes9x's and Supafaust's, the chip seated and in the
    // air between them
    let snes = omega_ui::SNES_SOCKETS;
    let snes_board = board_face(&Cart {
        platform: Platform::Snes,
        stem: "Super Metroid".into(),
        rom: "Games/SNES/Super Metroid.sfc".into(),
        code: String::new(),
        title: "SUPER METROID".into(),
        ..cart.clone()
    });
    let snes_socketed = |board: &CartFace| {
        let mut f = CartFace {
            rgba: board.rgba.clone(),
            w: board.w,
            h: board.h,
        };
        for (i, core) in Core::for_platform(Platform::Snes).iter().enumerate() {
            over(&mut f, &socket_face(*core), snes.u[i] as i32, snes.v as i32);
        }
        f
    };
    let mut seated = snes_socketed(&snes_board);
    over(
        &mut seated,
        &chip_face(Some(Core::Snes9x)),
        snes.chip_u(0) as i32,
        snes.chip_v() as i32,
    );
    both("board-snes-seated", &seated);
    let mut hop = snes_socketed(&snes_board);
    let u = f32::midpoint(snes.chip_u(0), snes.chip_u(1));
    over(
        &mut hop,
        &omega_ui::chip_shadow_face_in(snes),
        (u + SHADOW_AT[0]) as i32,
        (snes.chip_v() + SHADOW_AT[1]) as i32,
    );
    over(
        &mut hop,
        &omega_ui::chip_face_in(snes, None),
        u as i32,
        (snes.chip_v() - HOP_LIFT) as i32,
    );
    both("board-snes-hop", &hop);
    let mut faust = snes_socketed(&snes_board);
    over(
        &mut faust,
        &chip_face(Some(Core::Supafaust)),
        snes.chip_u(1) as i32,
        snes.chip_v() as i32,
    );
    both("board-snes-faust", &faust);

    for (name, code) in [("board-red", "AXV"), ("board-grey", "")] {
        let red = board_face(&Cart {
            code: code.into(),
            stem: "Metroid Fusion".into(),
            ..cart.clone()
        });
        let mut f = socketed(&red);
        over(
            &mut f,
            &chip_face(Some(Core::Gpsp)),
            CHIP_U[1] as i32,
            CHIP_V as i32,
        );
        both(name, &f);
    }

    // a SNES cart opens on a SNES board in its own shell, its two cores socketed
    for (name, stem) in [
        ("board-snes-ntsc", "Super Metroid (USA)"),
        ("board-snes-sfc", "Seiken Densetsu 3 (Japan)"),
    ] {
        let board = board_face(&Cart {
            platform: Platform::Snes,
            stem: stem.into(),
            rom: format!("Games/SNES/{stem}.sfc").into(),
            code: String::new(),
            ..cart.clone()
        });
        both(&format!("{name}-bare"), &board);
        let mut f = snes_socketed(&board);
        over(
            &mut f,
            &chip_face(Some(Core::Snes9x)),
            snes.chip_u(0) as i32,
            snes.chip_v() as i32,
        );
        both(name, &f);
    }
}
