use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Book {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub published: Option<i64>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub formats: Vec<Format>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Format {
    pub format: String,
    pub url: String,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Category {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub struct BooksResponse {
    pub books: Vec<Book>,
    #[serde(default)]
    pub total: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CategoriesResponse {
    pub categories: Vec<Category>,
}

#[derive(Debug, Deserialize)]
pub struct BookResponse {
    pub book: Book,
}

pub fn extension_for(format: &str) -> &str {
    match format {
        "epub" | "azw3" | "mobi" | "kepub" | "txt" | "html" => format,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_roundtrip_minimal_book() {
        let json = r#"{"id":"1","title":"T","formats":[]}"#;
        let book: Book = serde_json::from_str(json).unwrap();
        assert_eq!(book.id, "1");
        assert_eq!(book.title, "T");
        assert!(book.authors.is_empty());
        assert!(serde_json::to_string(&book).unwrap().contains("\"authors\":[]"));
    }

    #[test]
    fn serde_roundtrip_full_book() {
        let json = r#"{"id":"2","title":"T","authors":["A"],"languages":["en"],
            "published":1851,"description":"d","categories":["c"],
            "formats":[{"format":"epub","url":"https://x/y.epub","size":10}]}"#;
        let book: Book = serde_json::from_str(json).unwrap();
        assert_eq!(book.published, Some(1851));
        assert_eq!(book.formats[0].size, Some(10));
    }

    #[test]
    fn extension_for_known_and_unknown() {
        assert_eq!(extension_for("epub"), "epub");
        assert_eq!(extension_for("txt"), "txt");
        assert_eq!(extension_for("weird"), "weird");
    }
}
