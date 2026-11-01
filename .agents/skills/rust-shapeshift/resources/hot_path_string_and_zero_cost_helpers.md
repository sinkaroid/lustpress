# Hot-Path String Optimization & Zero-Cost Helpers

In dynamic languages (JavaScript / Python), developers habitually call regex replacements (`str.replace(/\s+/g, ' ')`), date libraries (Moment/Dayjs), or dynamic validators (Zod/Joi) inside request loops. In Rust, applying dynamic regex or heavy parsing in hot paths introduces significant CPU overhead and micro-allocations.

---

## 1. Zero-Regex Whitespace Squeezing (Single-Pass Iterator)

In `jandapress/src/scraper/pururin.rs`, collapsing newlines and multiple spaces into a single space is executed on every parsed tag.

Instead of running a regex engine, this single-pass character iterator performs zero regex compilation and minimal allocations:

```rust
pub fn squeeze_whitespace(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut last_was_space = false;

    for c in value.chars() {
        if c.is_whitespace() {
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(c);
            last_was_space = false;
        }
    }

    result.trim().to_string()
}
```

---

## 2. Integer Arithmetic Over Heavy Date/Time Crates

In `lustpress/src/util.rs`, converting video duration in seconds into `"{m}min, {s}sec"` format is called on hundreds of items per search request.

Instead of creating DateTime objects or calling heavy formatting libraries, use native CPU integer division and modulo:

```rust
#[inline]
pub fn second_to_minute(seconds: u64) -> String {
    let m = seconds / 60;
    let s = seconds % 60;
    format!("{m}min, {s}sec")
}
```

---

## 3. Safe URL Query Encoding (`form_urlencoded`)

In legacy JavaScript, query spacers and URL replacements (`val.replace(/ /g, '+')`) easily introduce injection vulnerabilities or malformed URLs.

Use `form_urlencoded::byte_serialize` from the `url` crate for zero-cost, standard-compliant query encoding:

```rust
use url::form_urlencoded;

#[inline]
pub fn encode_query(val: &str) -> String {
    form_urlencoded::byte_serialize(val.as_bytes()).collect()
}
```

---

## 4. Static Regex Compilation with `LazyLock<Regex>`

When regular expressions are strictly required (such as mathematical challenge extraction in `lustpress/src/fetch.rs`), **NEVER** construct `Regex::new(...)` inside function bodies.

Always compile regex patterns once into static memory:

```rust
use regex::Regex;
use std::sync::LazyLock;

static RE_CHALLENGE_P: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"var\s+p\s*=\s*(-?\d+)").unwrap());
static RE_CHALLENGE_S: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"var\s+s\s*=\s*(-?\d+)").unwrap());
static RE_IFELSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"if\(\(s>>(\d+)\)&1\)\{p([+-])=(\d+)\*(\d+);\}else\{p([+-])=(\d+)\*(\d+);\}").unwrap());
```

---

## 5. Zero-Cost Query Validation (`validator` Derive)

Instead of runtime validation libraries (like Zod) that build heap schemas on each request, use `validator::Validate` derive macros.

Validation executes as inline compile-time generated boolean checks with zero heap allocation when valid:

```rust
use axum::extract::Query;
use validator::Validate;

#[derive(serde::Deserialize, Validate)]
pub struct SearchParams {
    #[validate(length(min = 1, max = 100, message = "Query cannot be empty"))]
    pub q: String,

    #[validate(range(min = 1, max = 500, message = "Page out of bounds"))]
    pub page: Option<u32>,
}

pub async fn search_handler(Query(params): Query<SearchParams>) -> Result<(), AppError> {
    params.validate().map_err(|e| AppError::Validation(e.to_string()))?;
    Ok(())
}
```
