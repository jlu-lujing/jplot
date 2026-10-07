use jplot_core::scene::{Primitive, Scene, TextStyle, layer};
fn main() {
    let mut sc = Scene::new(300.0, 40.0);
    sc.layer(layer::AXES).push(Primitive::Text {
        content: "0123456789".into(),
        x: 5.0,
        y: 20.0,
        style: TextStyle { size: 8.8, ..Default::default() },
    });
    let png = jplot_render::to_png(&sc, 1.0).unwrap();
    std::fs::write("/tmp/texttest.png", png).unwrap();
    println!("wrote /tmp/texttest.png");
}
