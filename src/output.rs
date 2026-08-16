use crate::book::{Book, Category};

pub fn render_books_table(books: &[Book]) -> String {
    if books.is_empty() {
        return "no results\n".to_string();
    }
    let cols = ["TITLE", "AUTHOR(S)", "ID", "FORMATS"];
    let rows: Vec<[String; 4]> = books
        .iter()
        .map(|b| {
            [
                b.title.clone(),
                b.authors.join(", "),
                b.id.clone(),
                b.formats.iter().map(|f| f.format.clone()).collect::<Vec<_>>().join(", "),
            ]
        })
        .collect();
    let mut widths = [0usize; 4];
    for i in 0..4 {
        widths[i] = cols[i].len();
    }
    for r in &rows {
        for i in 0..4 {
            widths[i] = widths[i].max(r[i].len());
        }
    }
    let row = |cells: &[String; 4]| -> String {
        (0..4)
            .map(|i| format!("{:<w$}", cells[i], w = widths[i]))
            .collect::<Vec<_>>()
            .join("  ")
            + "\n"
    };
    let mut out = String::new();
    out.push_str(&row(&[
        "TITLE".into(),
        "AUTHOR(S)".into(),
        "ID".into(),
        "FORMATS".into(),
    ]));
    out.push_str(&row(&[
        "-----".into(),
        "---------".into(),
        "--".into(),
        "-------".into(),
    ]));
    for r in &rows {
        out.push_str(&row(r));
    }
    out
}

pub fn render_books_json(books: &[Book]) -> String {
    serde_json::to_string_pretty(books).unwrap() + "\n"
}

pub fn render_book(book: &Book) -> String {
    let mut out = format!("{}\n", book.title);
    if !book.authors.is_empty() {
        out.push_str(&format!("by {}\n", book.authors.join(", ")));
    }
    if let Some(p) = book.published {
        out.push_str(&format!("published {p}\n"));
    }
    out.push_str("formats:\n");
    if book.formats.is_empty() {
        out.push_str("  (none)\n");
    }
    for f in &book.formats {
        out.push_str(&format!("  {} - {}\n", f.format, f.url));
    }
    out
}

pub fn render_categories(cats: &[Category]) -> String {
    if cats.is_empty() {
        return "no categories\n".to_string();
    }
    let mut out = String::new();
    for c in cats {
        out.push_str(&format!("{} - {}\n", c.title, c.id));
    }
    out
}

/// Result count line per spec: `total`, when provided, enables "showing x of
/// y"; when absent the host prints "n shown".
pub fn render_count_line(total: Option<u64>, shown: usize) -> String {
    match total {
        Some(t) => format!("showing {shown} of {t}\n"),
        None => format!("{shown} shown\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> Book {
        Book {
            id: "1".into(),
            title: "Moby Dick".into(),
            authors: vec!["Herman Melville".into()],
            languages: vec![],
            published: Some(1851),
            description: None,
            categories: vec![],
            formats: vec![
                crate::book::Format { format: "epub".into(), url: "https://x/1.epub".into(), size: None },
                crate::book::Format { format: "txt".into(), url: "https://x/1.txt".into(), size: None },
            ],
        }
    }

    #[test]
    fn table_has_header_and_rows() {
        let out = render_books_table(&[book()]);
        assert!(out.contains("TITLE"));
        assert!(out.contains("Moby Dick"));
        assert!(out.contains("Herman Melville"));
        assert!(out.contains("epub, txt"));
    }

    #[test]
    fn empty_table_says_no_results() {
        assert_eq!(render_books_table(&[]), "no results\n");
    }

    #[test]
    fn json_output_is_parseable() {
        let out = render_books_json(&[book()]);
        let parsed: Vec<Book> = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed[0].title, "Moby Dick");
    }

    #[test]
    fn book_details_include_formats() {
        let out = render_book(&book());
        assert!(out.contains("Moby Dick"));
        assert!(out.contains("by Herman Melville"));
        assert!(out.contains("published 1851"));
        assert!(out.contains("  epub - https://x/1.epub"));
    }

    #[test]
    fn categories_render_title_and_id() {
        let out = render_categories(&[Category { id: "c1".into(), title: "New Releases".into() }]);
        assert!(out.contains("New Releases - c1"));
        assert_eq!(render_categories(&[]), "no categories\n");
    }

    #[test]
    fn count_line_shows_total_when_present() {
        assert_eq!(render_count_line(Some(42), 20), "showing 20 of 42\n");
        assert_eq!(render_count_line(None, 20), "20 shown\n");
    }
}
