#[test]
fn dbg_box() {
    use jplot_core::data::{Column, Dataset};
    use jplot_core::spec::{aes, geom_boxplot, ggplot};
    use std::collections::HashMap;
    let mut d = Dataset::new();
    d.add("g", Column::Categorical { values: vec!["a".into(); 10], levels: Some(vec!["a".into()]) });
    d.add("v", Column::Numeric { values: (1..=10).map(|i| i as f64).collect(), label: None });
    let mut m = HashMap::new();
    m.insert("x".into(), "g".into());
    m.insert("y".into(), "v".into());
    let p = ggplot(d) + aes(m) + geom_boxplot();
    let b = jplot_core::build::build(&p.spec).unwrap();
    println!("frame num: {:?}", b.layers[0].frame.num.iter().map(|(k,v)| (k.clone(), v.clone())).collect::<Vec<_>>());
    println!("frame cat: {:?}", b.layers[0].frame.cat);
    let sc = jplot_core::layout::layout(&b);
    for l in &sc.layers {
        for pr in &l.primitives {
            println!("{pr:?}");
        }
    }
}
