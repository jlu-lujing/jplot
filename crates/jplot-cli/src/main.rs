use std::process::ExitCode;

use jplot_core::spec::PlotSpec;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "render-svg" {
        return render_svg_cmd(&args);
    }
    if args.len() < 3 || args[1] != "render" {
        eprintln!("usage: jplot render <spec.json> [-o out.(svg|png)] [--scale f]");
        eprintln!("   or: jplot render-svg <in.svg> -o <out.png> [--scale f]");
        return ExitCode::from(2);
    }
    let spec_path = &args[2];
    let mut out: Option<String> = None;
    let mut scale = 2.0;
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                out = args.get(i).cloned();
            }
            "--scale" => {
                i += 1;
                scale = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(2.0);
            }
            _ => {}
        }
        i += 1;
    }
    let raw = std::fs::read_to_string(spec_path).unwrap_or_else(|e| {
        eprintln!("cannot read {spec_path}: {e}");
        std::process::exit(2)
    });
    let spec: PlotSpec = match serde_json::from_str(&raw) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("invalid spec: {e}");
            return ExitCode::from(1);
        }
    };
    run(&spec, out.as_deref(), spec_path, scale)
}

fn render_svg_cmd(args: &[String]) -> ExitCode {
    if args.len() < 3 {
        eprintln!("usage: jplot render-svg <in.svg> -o <out.png>");
        return ExitCode::from(2);
    }
    let svg_path = &args[2];
    let mut out: Option<String> = None;
    let mut scale = 1.0;
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                out = args.get(i).cloned();
            }
            "--scale" => {
                i += 1;
                scale = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(1.0);
            }
            _ => {}
        }
        i += 1;
    }
    let svg = std::fs::read_to_string(svg_path).unwrap_or_else(|e| {
        eprintln!("cannot read {svg_path}: {e}");
        std::process::exit(2)
    });
    let out = out.unwrap_or_else(|| svg_path.replace(".svg", ".png"));
    match jplot_render::svg_to_png(&svg, scale) {
        Ok(png) => {
            if let Err(e) = std::fs::write(&out, png) {
                eprintln!("write {out}: {e}");
                return ExitCode::from(1);
            }
            eprintln!("wrote {out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("rasterise failed: {e}");
            ExitCode::from(1)
        }
    }
}

fn run(spec: &PlotSpec, out: Option<&str>, spec_path: &str, scale: f64) -> ExitCode {
    let built = match jplot_core::build(spec) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("build error: {e}");
            return ExitCode::from(1);
        }
    };
    let scene = jplot_core::layout(&built);
    let svg = jplot_render::to_svg(&scene);
    let dest = out.map(|s| s.to_string()).unwrap_or_else(|| {
        spec_path.rsplit_once('.').map(|(p, _)| format!("{p}.svg")).unwrap_or_else(|| spec_path.to_string() + ".svg")
    });
    if dest.ends_with(".png") {
        let png = jplot_render::to_png(&scene, scale).unwrap_or_else(|e| {
            eprintln!("png render failed: {e}");
            std::process::exit(1)
        });
        if let Err(e) = std::fs::write(&dest, png) {
            eprintln!("write {dest}: {e}");
            return ExitCode::from(1);
        }
    } else {
        if let Err(e) = std::fs::write(&dest, svg) {
            eprintln!("write {dest}: {e}");
            return ExitCode::from(1);
        }
    }
    eprintln!("wrote {dest}");
    ExitCode::SUCCESS
}
