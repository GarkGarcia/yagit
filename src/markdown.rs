use std::io::{self, Write};
use crate::{BLOB_SUBDIR, Escaped};
use pulldown_cmark::{Parser, Options, Event, Tag, TagEnd, LinkType};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct State {
  in_non_writing_block: bool,
  in_table_head: bool,
}

// Addapted from pulldown_cmark/html.rs
pub fn render_html<W: Write>(w: &mut W, src: &String) -> io::Result<()> {
  let mut opt = Options::empty();
  opt.insert(Options::ENABLE_TABLES);
  opt.insert(Options::ENABLE_STRIKETHROUGH);
  opt.insert(Options::ENABLE_TASKLISTS);
  opt.insert(Options::ENABLE_SMART_PUNCTUATION);
  opt.insert(Options::ENABLE_DEFINITION_LIST);
  opt.insert(Options::ENABLE_SUPERSCRIPT);
  opt.insert(Options::ENABLE_SUBSCRIPT);

  let mut p = Parser::new_ext(src.as_ref(), opt);
  let mut state = State {
    in_non_writing_block: false,
    in_table_head: true,
  };

  while let Some(event) = p.next() {
    match event {
      Event::Start(tag) => start_tag(w, tag, &mut state, &mut p)?,
      Event::End(tag)   => end_tag(w, tag, &mut state)?,
      Event::Text(text) => if !state.in_non_writing_block {
        if text.ends_with('\n') {
          write!(w, "{}", Escaped(&text))?;
        } else {
          writeln!(w, "{}", Escaped(&text))?;
        }
      },
      Event::Code(text) => write!(w, "<code>{}</code>", Escaped(&text))?,
      Event::InlineMath(_) => {
        unreachable!("inline math is not supported");
      }
      Event::DisplayMath(_) => {
        unreachable!("display math is not supported");
      }
      Event::SoftBreak => writeln!(w)?,
      Event::HardBreak => writeln!(w, "<br />")?,
      Event::Rule      => writeln!(w, "<hr />")?,
      Event::TaskListMarker(true) => {
        writeln!(w, "<input disabled=\"\" type=\"checkbox\" checked=\"\"/>")?;
      }
      Event::TaskListMarker(false) => {
        writeln!(w, "<input disabled=\"\" type=\"checkbox\"/>")?;
      }
      Event::Html(_) | Event::InlineHtml(_) => {} // running in safe mode
      Event::FootnoteReference(_) => {
        unreachable!("footnotes are not supported");
      }
    }
  }
  Ok(())
}

// Addapted from pulldown_cmark/html.rs
/// Returns `Ok(t)` if successful,
/// where `t` indicates whether or not we are in a non-writting block
fn start_tag<W: Write>(
  w: &mut W,
  tag: Tag<'_>,
  state: &mut State,
  p: &mut Parser,
) -> io::Result<()> {
  match tag {
    Tag::HtmlBlock => {
      // runing in safe mode
      state.in_non_writing_block = true;
    }
    Tag::Paragraph             => writeln!(w, "<p>")?,
    Tag::Heading { level, .. } => write!(w, "<{level}>")?,
    Tag::Subscript             => write!(w, "<sub>")?,
    Tag::Superscript           => write!(w, "<sup>")?,
    Tag::Table(_alignments)    => write!(w, "<table>")?,
    Tag::TableHead => {
      state.in_table_head = true;
      write!(w, "<thead><tr>")?;
    }
    Tag::TableRow => write!(w, "<tr>")?,
    Tag::TableCell => if state.in_table_head {
      write!(w, "<th>")?;
    } else {
      write!(w, "<td>")?;
    },
    Tag::CodeBlock(_) => {
      writeln!(w, "<div class=\"code-block\">")?;
      write!(w, "<pre>")?;
    }
    Tag::BlockQuote(_)            => writeln!(w, "<blockquote>")?,
    Tag::List(Some(1))            => writeln!(w, "<ol>")?,
    Tag::List(Some(start))        => writeln!(w, "<ol start=\"{start}\">")?,
    Tag::List(None)               => writeln!(w, "<ul>")?,
    Tag::Item                     => writeln!(w, "<li>")?,
    Tag::DefinitionList           => writeln!(w, "<dl>")?,
    Tag::DefinitionListTitle      => write!(w, "<dt>")?,
    Tag::DefinitionListDefinition => write!(w, "<dd>")?,
    Tag::Emphasis                 => write!(w, "<em>")?,
    Tag::Strong                   => write!(w, "<strong>")?,
    Tag::Strikethrough            => write!(w, "<del>")?,
    Tag::Link { link_type: LinkType::Email, dest_url, .. } => {
      write!(w, "<a href=\"mailto:{url}\">", url = Escaped(&dest_url))?;
    }
    Tag::Link { dest_url, .. } => {
      write!(w, "<a href=\"{url}\">", url = Escaped(&dest_url))?;
    }
    Tag::Image { dest_url, title, .. } => {
      if dest_url.starts_with("https://") || dest_url.starts_with("http://") {
        write!(w, "<img src=\"{url}\" ", url = Escaped(&dest_url))?;
      } else {
        // relative URL
        write!(w, "<img src=\"/{BLOB_SUBDIR}/{url}\" ",
                  url = Escaped(&dest_url))?;
      };

      if let Some(Event::Text(alt)) = p.next() {
        write!(w, "alt=\"{alt}\" ", alt = Escaped(&alt))?;
      } 

      if !title.is_empty() {
        write!(w, "title=\"{title}\" ", title = Escaped(&title))?;
      }

      writeln!(w, "/>")?;
    }
    Tag::FootnoteDefinition(_) => {
      unreachable!("footnotes are not supported");
    }
    Tag::MetadataBlock(_) => {
      unreachable!("metadata blocks are not supported");
    }
  }

  Ok(())
}

// Addapted from pulldown_cmark/html.rs
/// Returns `Ok(t)` if successful,
/// where `t` indicates whether or not we are in a non-writting block
fn end_tag<W: Write>(
  w: &mut W,
  tag: TagEnd,
  state: &mut State,
) -> io::Result<()> {
  match tag {
    TagEnd::HtmlBlock => {
      // runing in safe mode
      state.in_non_writing_block = false;
    }
    TagEnd::Paragraph      => writeln!(w, "</p>")?,
    TagEnd::Heading(level) => writeln!(w, "</{level}>")?,
    TagEnd::Subscript      => write!(w, "</sub>")?,
    TagEnd::Superscript    => write!(w, "</sup>")?,
    TagEnd::Table => {
      writeln!(w, "</tbody>")?;
      writeln!(w, "</table>")?;
    }
    TagEnd::TableHead => {
      writeln!(w, "</tr>")?;
      writeln!(w, "</thead>")?;
      writeln!(w, "<tbody>")?;
      state.in_table_head = false;
    }
    TagEnd::TableRow => writeln!(w, "</tr>")?,
    TagEnd::TableCell => if state.in_table_head {
      write!(w, "</th>")?;
    } else {
      write!(w, "</td>")?;
    },
    TagEnd::CodeBlock => {
      writeln!(w, "</pre>")?;
      writeln!(w, "</div>")?;
    }
    TagEnd::BlockQuote(_)            => writeln!(w, "</blockquote>")?,
    TagEnd::List(true)               => writeln!(w, "</ol>")?,
    TagEnd::List(false)              => writeln!(w, "</ul>")?,
    TagEnd::Item                     => writeln!(w, "</li>")?,
    TagEnd::DefinitionList           => writeln!(w, "</dl>")?,
    TagEnd::DefinitionListTitle      => writeln!(w, "</dt>")?,
    TagEnd::DefinitionListDefinition => writeln!(w, "</dd>")?,
    TagEnd::Emphasis                 => write!(w, "</em>")?,
    TagEnd::Strong                   => write!(w, "</strong>")?,
    TagEnd::Strikethrough            => write!(w, "</del>")?,
    TagEnd::Link                     => write!(w, "</a>")?,
    TagEnd::Image                    => {} // handled in start_tag
    TagEnd::FootnoteDefinition => {
      unreachable!("footnotes are not supported");
    }
    TagEnd::MetadataBlock(_) => {
      unreachable!("metadata blocks are not supported");
    }
  }

  Ok(())
}

