//! A BPMN document read into elements: each with its namespace resolved,
//! its attributes, its text and where it stands in the bytes, so the
//! compiler reads a tree and a designer's edit can be placed exactly.
//!
//! Read with quick-xml's namespace-resolving pull parser. A namespace is a
//! name, compared and never fetched; a document type declaration is
//! refused, so no entity of anyone else's is ever expanded or asked for.

use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;

/// One element.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Element {
    /// Its namespace, empty where it has none.
    pub namespace: String,
    /// Its local name.
    pub name: String,
    /// Its attributes: namespace, local name and value, in order.
    pub attributes: Vec<(String, String, String)>,
    pub children: Vec<Element>,
    /// The text directly inside it, entities read.
    pub text: String,
    /// Where it starts and ends in the document's bytes.
    pub start: usize,
    pub end: usize,
    /// Where its start tag ends and where its end tag starts: what it holds
    /// lies between. Both are its end where it is empty.
    pub open_end: usize,
    pub close_start: usize,
}

impl Element {
    /// The attribute `name` without a namespace, or `None`.
    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(namespace, local, _)| namespace.is_empty() && local == name)
            .map(|(_, _, value)| value.as_str())
    }

    /// The attribute `name` in `namespace`, or `None`.
    #[must_use]
    pub fn attribute_in(&self, namespace: &str, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(space, local, _)| space == namespace && local == name)
            .map(|(_, _, value)| value.as_str())
    }

    /// Whether it is `name` in `namespace`.
    #[must_use]
    pub fn is(&self, namespace: &str, name: &str) -> bool {
        self.namespace == namespace && self.name == name
    }

    /// Its children that are `name` in `namespace`.
    pub fn children_named<'a>(
        &'a self,
        namespace: &'a str,
        name: &'a str,
    ) -> impl Iterator<Item = &'a Element> {
        self.children
            .iter()
            .filter(move |child| child.is(namespace, name))
    }

    /// Its first child that is `name` in `namespace`.
    #[must_use]
    pub fn child(&self, namespace: &str, name: &str) -> Option<&Element> {
        self.children.iter().find(|child| child.is(namespace, name))
    }

    /// Its identifier, or its name where it has none.
    #[must_use]
    pub fn label(&self) -> &str {
        self.attribute("id").unwrap_or(&self.name)
    }
}

/// `text` read into its root element.
///
/// # Errors
/// The text is not well-formed XML, holds a document type declaration, or
/// names a prefix it never bound — each said with where.
pub fn read(text: &str) -> Result<Element, String> {
    let mut reader = NsReader::from_str(text);
    let mut open: Vec<Element> = Vec::new();
    loop {
        let before = usize::try_from(reader.buffer_position()).unwrap_or(usize::MAX);
        let (resolved, event) = reader
            .read_resolved_event()
            .map_err(|why| format!("at byte {before}: {why}"))?;
        let namespace = namespace_of(&resolved, before)?;
        let after = usize::try_from(reader.buffer_position()).unwrap_or(usize::MAX);
        match event {
            Event::Start(start) => {
                let mut started = element(&reader, &start, namespace, before)?;
                started.open_end = after;
                open.push(started);
            }
            Event::Empty(start) => {
                let mut empty = element(&reader, &start, namespace, before)?;
                empty.end = after;
                empty.open_end = after;
                empty.close_start = after;
                if let Some(done) = close(&mut open, empty) {
                    return Ok(done);
                }
            }
            Event::End(_) => {
                let Some(mut done) = open.pop() else {
                    return Err(format!("at byte {before}: an end with no start"));
                };
                done.end = after;
                done.close_start = before;
                if let Some(root) = close(&mut open, done) {
                    return Ok(root);
                }
            }
            Event::Text(text) => {
                if let Some(current) = open.last_mut() {
                    current.text.push_str(&text.xml10_content());
                }
            }
            Event::CData(data) => {
                if let Some(current) = open.last_mut() {
                    current.text.push_str(&data.xml10_content());
                }
            }
            Event::GeneralRef(reference) => {
                let name = reference.into_inner();
                let read = codec::xml::unescape(&format!("&{name};"))
                    .map_err(|why| format!("at byte {before}: {why}"))?;
                if let Some(current) = open.last_mut() {
                    current.text.push_str(&read);
                }
            }
            Event::DocType(_) => {
                return Err(format!(
                    "at byte {before}: a document type declaration, which a BPMN design \
                     never needs and Xmip never reads"
                ));
            }
            Event::Eof => return Err("the document ends before its root element".to_string()),
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) => {}
        }
    }
}

/// `done` added to the element it stands in, or returned where it is the
/// root.
fn close(open: &mut [Element], done: Element) -> Option<Element> {
    match open.last_mut() {
        Some(parent) => {
            parent.children.push(done);
            None
        }
        None => Some(done),
    }
}

fn namespace_of(resolved: &ResolveResult<'_>, at: usize) -> Result<String, String> {
    match resolved {
        ResolveResult::Bound(namespace) => Ok(namespace.0.to_string()),
        ResolveResult::Unbound => Ok(String::new()),
        ResolveResult::Unknown(prefix) => Err(format!(
            "at byte {at}: the prefix '{prefix}' is bound to no namespace"
        )),
    }
}

fn element(
    reader: &NsReader<&[u8]>,
    start: &BytesStart<'_>,
    namespace: String,
    at: usize,
) -> Result<Element, String> {
    let mut attributes = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|why| format!("at byte {at}: {why}"))?;
        let (resolved, local) = reader.resolver().resolve_attribute(attribute.key);
        let space = namespace_of(&resolved, at)?;
        let value = attribute
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|why| format!("at byte {at}: {why}"))?;
        attributes.push((space, local.as_ref().to_string(), value.into_owned()));
    }
    Ok(Element {
        namespace,
        name: start.local_name().as_ref().to_string(),
        attributes,
        start: at,
        ..Element::default()
    })
}
