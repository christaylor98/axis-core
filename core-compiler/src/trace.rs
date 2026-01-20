// AXIS_TRACE=1 gated debug tracing (stderr only)

pub fn trace(msg: &str) {
    if std::env::var("AXIS_TRACE").is_ok() {
        eprintln!("[TRACE] {}", msg);
    }
}
