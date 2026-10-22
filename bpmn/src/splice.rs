//! Text placed into a BPMN document where an element's bytes say, so an
//! edit changes what it means to and nothing else: foreign extensions,
//! comments and layout are left as written.

use crate::document::Element;

/// The element `id` names, at any depth.
#[must_use]
pub fn find<'a>(root: &'a Element, id: &str) -> Option<&'a Element> {
    if root.attribute("id") == Some(id) {
        return Some(root);
    }
    root.children.iter().find_map(|child| find(child, id))
}

/// The element holding the element `id`.
#[must_use]
pub fn parent_of<'a>(root: &'a Element, id: &str) -> Option<&'a Element> {
    if root.children.iter().any(|c| c.attribute("id") == Some(id)) {
        return Some(root);
    }
    root.children.iter().find_map(|child| parent_of(child, id))
}

/// Every element at any depth, in document order.
pub fn every(root: &Element) -> Vec<&Element> {
    let mut all = vec![root];
    for child in &root.children {
        all.extend(every(child));
    }
    all
}

/// The prefix `root` binds `namespace` to: empty for the default namespace,
/// `None` where it binds none.
#[must_use]
pub fn prefix(root: &Element, namespace: &str) -> Option<String> {
    root.attributes.iter().find_map(|(space, local, value)| {
        if value != namespace {
            return None;
        }
        if space.is_empty() && local == "xmlns" {
            return Some(String::new());
        }
        space.contains("xmlns").then(|| local.clone())
    })
}

/// `name` written with `prefix`.
#[must_use]
pub fn qualified(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}:{name}")
    }
}

/// `text` with `at..until` made `with`.
#[must_use]
pub fn replaced(text: &str, (at, until): (usize, usize), with: &str) -> String {
    let mut out = String::with_capacity(text.len() + with.len());
    out.push_str(&text[..at]);
    out.push_str(with);
    out.push_str(&text[until..]);
    out
}

/// The white space a line starts with before `at`.
#[must_use]
pub fn indent_of(text: &str, at: usize) -> String {
    let line = text[..at].rfind('\n').map_or(0, |newline| newline + 1);
    text[line..at]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect()
}

/// The name an element is written with, its prefix included.
#[must_use]
pub fn written_name<'a>(text: &'a str, element: &Element) -> &'a str {
    let rest = &text[element.start + 1..element.open_end];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(rest.len());
    &rest[..end]
}

/// `snippet` placed as `parent`'s last child, indented one step past it.
#[must_use]
pub fn appended(text: &str, parent: &Element, snippet: &str) -> String {
    let indent = indent_of(text, parent.start);
    if parent.open_end == parent.end {
        // An empty element, `<a/>`, opened to hold it.
        let name = written_name(text, parent);
        let close = parent.end - 2;
        let opened = format!(">\n{indent}  {snippet}\n{indent}</{name}>");
        return replaced(text, (close, parent.end), &opened);
    }
    let before = &text[..parent.close_start];
    let on_its_own_line = before.trim_end_matches([' ', '\t']).ends_with('\n');
    let placed = if on_its_own_line {
        format!("  {snippet}\n{indent}")
    } else {
        format!("\n{indent}  {snippet}\n{indent}")
    };
    replaced(text, (parent.close_start, parent.close_start), &placed)
}

/// `snippet` placed as `parent`'s first child.
#[must_use]
pub fn prepended(text: &str, parent: &Element, snippet: &str) -> String {
    if parent.open_end == parent.end {
        return appended(text, parent, snippet);
    }
    let indent = indent_of(text, parent.start);
    let placed = format!("\n{indent}  {snippet}");
    replaced(text, (parent.open_end, parent.open_end), &placed)
}

/// `element` taken out, with the line it stood on where it stood alone.
#[must_use]
pub fn removed(text: &str, element: &Element) -> String {
    let line = text[..element.start].rfind('\n').map_or(0, |n| n + 1);
    let alone_before = text[line..element.start].trim().is_empty();
    let after = &text[element.end..];
    let rest = after.find('\n').map_or(after.len(), |n| n + 1);
    let alone_after = after[..rest].trim().is_empty();
    if alone_before && alone_after && line > 0 {
        replaced(text, (line - 1, element.end + rest - 1), "")
    } else {
        replaced(text, (element.start, element.end), "")
    }
}

/// `element`'s attribute `name` set to `value`, or taken out where `value`
/// is `None`.
#[must_use]
pub fn attributed(text: &str, element: &Element, name: &str, value: Option<&str>) -> String {
    let tag = &text[element.start..element.open_end];
    let found = [format!(" {name}=\""), format!(" {name}='")]
        .into_iter()
        .find_map(|opening| tag.find(&opening).map(|at| (at, opening)));
    if let Some((at, opening)) = found {
        let quote = opening.chars().last().unwrap_or('"');
        let value_at = element.start + at + opening.len();
        let close = text[value_at..]
            .find(quote)
            .map_or(value_at, |n| value_at + n);
        return match value {
            Some(value) => replaced(text, (value_at, close), &codec::xml::escape(value)),
            None => replaced(text, (element.start + at, close + 1), ""),
        };
    }
    let Some(value) = value else {
        return text.to_string();
    };
    let end = if element.open_end == element.end && tag.ends_with("/>") {
        element.open_end - 2
    } else {
        element.open_end - 1
    };
    let attribute = format!(" {name}=\"{}\"", codec::xml::escape(value));
    replaced(text, (end, end), &attribute)
}

/// An identifier no element of `root` has, starting with `base`.
#[must_use]
pub fn unused_id(root: &Element, base: &str) -> String {
    let ids: Vec<&str> = every(root)
        .iter()
        .filter_map(|e| e.attribute("id"))
        .collect();
    (1..)
        .map(|n| format!("{base}_{n}"))
        .find(|id| !ids.contains(&id.as_str()))
        .unwrap_or_else(|| base.to_string())
}
