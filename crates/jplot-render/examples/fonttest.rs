use usvg::fontdb::Database;
fn main() {
    let mut db = Database::new();
    for p in ["/System/Library/Fonts/Helvetica.ttc", "/System/Library/Fonts/HelveticaNeue.ttc", "/System/Library/Fonts/Supplemental/Arial.ttf"] {
        match db.load_font_file(p) {
            Ok(()) => println!("loaded {p}"),
            Err(e) => println!("FAILED {p}: {e:?}"),
        }
    }
    for f in db.faces() {
        println!("face: {} (family={})", f.post_script_name, f.families.iter().map(|(n,_)| n.clone()).collect::<Vec<_>>().join("|"));
    }
}
