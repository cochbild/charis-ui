//! The GPU renderer must match the CPU reference renderer. Skipped when no
//! GPU adapter (hardware or software, e.g. lavapipe) is available.
#![cfg(feature = "gpu")]

use rust_ui::gpu::GpuRenderer;
use rust_ui::prelude::*;

struct Gallery;

#[derive(Clone)]
enum Msg {}

impl App for Gallery {
    type Msg = Msg;
    fn update(&mut self, _: Msg, _: &mut Cx) {}
    fn view(&self) -> Element<Msg> {
        let th = theme();
        col()
            .size_full()
            .p(24.0)
            .gap(16.0)
            .child(
                row()
                    .gap(12.0)
                    .child(primary_button("Primary"))
                    .child(button("Secondary"))
                    .child(danger_button("Danger"))
                    .child(switch(true))
                    .child(checkbox("Checked", true)),
            )
            .child(card().w(320.0).child(text("Card with a layered shadow").semibold()).child(progress(0.6)))
            .child(
                div().w(240.0).h(60.0).rounded(14.0).gradient(90.0, [(0.0, th.colors.accent), (1.0, hex("#a371f7"))]),
            )
            .child(
                div()
                    .w(200.0)
                    .h(80.0)
                    .clip()
                    .rounded(10.0)
                    .border(2.0, th.colors.border_strong)
                    .child(text("Clipped text that runs well past the edge of its rounded box").nowrap()),
            )
            .child(
                row().gap(10.0).children(
                    [Icon::Search, Icon::Settings, Icon::GitBranch, Icon::Star].map(|i| icon(i).font_size(20.0)),
                ),
            )
    }
}

#[test]
fn gpu_matches_cpu() {
    let Some(mut gpu) = GpuRenderer::headless() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let mut h = Headless::new(Gallery, 640.0, 420.0, 1.0);
    h.settle();
    let cpu = h.rt.render().clone();
    h.rt.render_scene();
    let g = h.rt.with_scene(|s, t| gpu.render_to_pixmap(s, t)).flatten().expect("gpu render");
    assert_eq!((cpu.width(), cpu.height()), (g.width(), g.height()));
    let (mut sum, mut big) = (0u64, 0usize);
    for (a, b) in cpu.data().chunks_exact(4).zip(g.data().chunks_exact(4)) {
        let d = (0..3).map(|i| (a[i] as i32 - b[i] as i32).unsigned_abs()).max().unwrap();
        sum += d as u64;
        if d > 48 {
            big += 1;
        }
    }
    let n = (cpu.width() * cpu.height()) as u64;
    let mean = sum as f64 / n as f64;
    assert!(mean < 0.5, "mean channel difference {mean}");
    assert!((big as f64) < n as f64 * 0.001, "{big} strongly differing pixels");
    eprintln!("gpu vs cpu: mean diff {mean:.3}, {big} pixels > 48");
}
