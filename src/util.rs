use url::form_urlencoded;

/// Safe URL query encoding (replaces legacy vulnerable spacer)
pub fn encode_query(val: &str) -> String {
    form_urlencoded::byte_serialize(val.as_bytes()).collect()
}

/// Converts seconds to "{m}min, {s}sec" format matching Pornhub legacy output
pub fn second_to_minute(seconds: u64) -> String {
    let m = seconds / 60;
    let s = seconds % 60;
    format!("{}min, {}sec", m, s)
}

/// Format current date in "MM/DD/YYYY, hh:mm:ss AM/PM" to match JS toLocaleString()
pub fn format_locale_date() -> String {
    let now = chrono::Local::now();
    now.format("%m/%d/%Y, %I:%M:%S %p").to_string()
}

/// Release unused glibc allocator heap pages back to the Linux kernel
#[inline]
pub fn trim_memory() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    unsafe {
        libc::malloc_trim(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_query() {
        assert_eq!(encode_query("hello world"), "hello+world");
        assert_eq!(encode_query("tag & category#1"), "tag+%26+category%231");
    }

    #[test]
    fn test_second_to_minute() {
        assert_eq!(second_to_minute(125), "2min, 5sec");
        assert_eq!(second_to_minute(60), "1min, 0sec");
    }
}
