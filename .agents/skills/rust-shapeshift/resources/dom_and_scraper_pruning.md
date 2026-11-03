# Scraper & DOM Optimization Reference: Cutting 80% Memory Bloat

Parsing full, unconstrained HTML documents (containing thousands of script tags, ads, comments, and footers) into an in-memory DOM tree allocates millions of transient AST nodes, severely stressing the memory allocator and bloating process RSS.

This reference guide demonstrates the exact patterns used to achieve sub-50MB RSS memory baselines by pruning raw text strings before AST instantiation.

---

## 1. Pattern A: Pre-Parse HTML Slicing

Never pass a full 5MB–10MB HTML page to `Html::parse_document` when target data is only in the first 20% of the page. Slicing the raw text string *before* parsing prevents the parser from generating thousands of unnecessary DOM nodes.

### Detail View vs Search / Related Pruning Strategies

```rust
use scraper::{Html, Selector};
use std::sync::LazyLock;

static SEL_SEARCH_WRAP: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.wrap").unwrap());
static SEL_SEARCH_TITLE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("a[title]").unwrap());

pub fn parse_search_optimized(body: &[u8], url: &str) -> Result<Vec<String>, String> {
    let html_str = String::from_utf8_lossy(body);
    let is_related = url.contains("view_video.php");

    // CRITICAL PRUNING BOUNDARIES:
    // For search listings: prune at pagination or footer.
    // For related videos: do not prune at pagination if comment pagination precedes the related video block.
    let end_pos = if is_related {
        html_str
            .find("<footer")
            .or_else(|| html_str.find("class=\"footer-wrap\""))
            .unwrap_or(html_str.len())
    } else {
        html_str
            .find("class=\"page_numbers\"")
            .or_else(|| html_str.find("class=\"pagination\""))
            .or_else(|| html_str.find("<footer"))
            .or_else(|| html_str.find("class=\"footer-wrap\""))
            .unwrap_or(html_str.len())
    };

    // Parse ONLY the sliced substring:
    let document = Html::parse_document(&html_str[..end_pos]);

    let mut titles = Vec::new();
    for el in document.select(&SEL_SEARCH_WRAP) {
        if let Some(title_el) = el.select(&SEL_SEARCH_TITLE).next() {
            if let Some(title) = title_el.value().attr("title") {
                titles.push(title.to_string());
            }
        }
    }

    // `document` drops immediately here, freeing all DOM tree nodes from RAM.
    Ok(titles)
}
```

---

## 2. Pattern B: Compile-Once Static Selectors (`LazyLock`)

Compiling CSS selector strings to AST on every request is CPU and memory inefficient. Use `std::sync::LazyLock` (native in modern Rust 1.80+):

```rust
use scraper::Selector;
use std::sync::LazyLock;

// Compiled exactly once during the entire runtime of the binary
static SEL_CANONICAL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("link[rel='canonical']").unwrap());
static SEL_OG_TITLE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("meta[property='og:title']").unwrap());
static SEL_OG_IMAGE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("meta[property='og:image']").unwrap());
static SEL_OG_DURATION: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("meta[property='video:duration']").unwrap());
static SEL_VIEWS: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.views > span.count").unwrap());
```

---

## 3. Pattern C: Zero-Allocation Byte Slice Parsing for JSON APIs

When scraping upstream providers that offer undocumented or internal JSON endpoints, **never** allocate an intermediate `String` from the network response.

Deserialize directly from `&[u8]` using `serde_json::from_slice`:

```rust
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ApiResponse {
    pub id: String,
    pub title: Option<String>,
    pub views: Option<u64>,
}

pub fn parse_direct_slice(bytes: &[u8]) -> Result<ApiResponse, serde_json::Error> {
    // 0 intermediate String allocations; borrows directly from byte buffer
    serde_json::from_slice(bytes)
}
```

---

## 4. Pattern D: Regression Unit Testing for Pruning Boundaries

Always provide mock HTML tests to verify that your string pruning logic does not prematurely chop valid content cards:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_pruning_boundary() {
        let mock_html = r##"
        <html>
        <body>
            <div class="wrap"><a title="Video 1"></a></div>
            <div class="pagination">Page 1, 2, 3</div>
            <div class="unneeded-content">Heavy comments and footer links</div>
            <footer>Footer</footer>
        </body>
        </html>
        "##;

        let results = parse_search_optimized(mock_html.as_bytes(), "https://example.com/search")
            .expect("Parsing should succeed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "Video 1");
    }

    #[test]
    fn test_related_pruning_with_comment_pagination() {
        let mock_html = r##"
        <html>
        <body>
            <div id="comments"><div class="pagination">Comment page 1</div></div>
            <div class="wrap"><a title="Related Video 1"></a></div>
            <footer>Footer</footer>
        </body>
        </html>
        "##;

        let results = parse_search_optimized(mock_html.as_bytes(), "https://example.com/view_video.php")
            .expect("Parsing should succeed without truncation from comment pagination");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "Related Video 1");
    }
}
```
