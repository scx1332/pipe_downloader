use human_bytes::human_bytes;

pub fn bytes_to_human(bytes: usize) -> String {
    human_bytes(bytes as f64)
}
