use ferrous_output::es_mux::{EsKind, EsMux};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let hevc = a[2] == "hevc";
    // split on AUD start codes
    let mut starts = vec![];
    let mut i = 0;
    while i + 4 < data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            let t = if hevc { (data[i + 3] >> 1) & 0x3f } else { data[i + 3] & 0x1f };
            if (hevc && t == 35) || (!hevc && t == 9) {
                let s = if i > 0 && data[i - 1] == 0 { i - 1 } else { i };
                starts.push(s);
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    starts.push(data.len());
    let mut mux = EsMux::new(if hevc { EsKind::Hevc } else { EsKind::H264 }, 60, (1280, 720));
    let mut out = vec![];
    for (n, w) in starts.windows(2).enumerate() {
        let au = &data[w[0]..w[1]];
        mux.frame(au, n as u64, n as i64, n % 60 == 0, &mut out);
    }
    std::fs::write(&a[3], out).unwrap();
    eprintln!("{} frames", starts.len() - 1);
}
