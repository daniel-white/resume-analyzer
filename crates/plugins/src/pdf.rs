//! Geometry-aware text extraction. PDF text runs can split inside a word;
//! a run boundary alone is not evidence of a word boundary.
use pdf_extract::{Document, MediaBox, OutputDev, OutputError, Transform};

#[derive(Default)]
struct TextOutput {
    text: String,
    previous: Option<(f64, f64)>,
    sizes: Vec<f32>,
    line_size: f32,
}

impl OutputDev for TextOutput {
    fn begin_page(
        &mut self,
        _: u32,
        _: &MediaBox,
        _: Option<(f64, f64, f64, f64)>,
    ) -> Result<(), OutputError> {
        if !self.text.is_empty() {
            self.text.push('\n');
            self.sizes.push(self.line_size);
            self.line_size = 0.;
        }
        self.previous = None;
        Ok(())
    }

    fn end_page(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn begin_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_line(&mut self) -> Result<(), OutputError> {
        Ok(())
    }

    fn output_character(
        &mut self,
        trm: &Transform,
        width: f64,
        _: f64,
        font_size: f64,
        character: &str,
    ) -> Result<(), OutputError> {
        let scale = trm.m11.hypot(trm.m12) * font_size.abs();
        let (x, y) = (trm.m31, trm.m32);
        if let Some((end, baseline)) = self.previous {
            if (y - baseline).abs() > scale * 0.5 {
                self.text.push('\n');
                self.sizes.push(self.line_size);
                self.line_size = 0.;
            // Preserve modest word gaps as well as large ones. In particular,
            // an initial capital is not enough evidence to join two words.
            } else if x - end > scale * 0.18
                && !self.text.ends_with(char::is_whitespace)
                && !character.starts_with(char::is_whitespace)
            {
                self.text.push(' ');
            }
        }
        self.line_size = self.line_size.max(scale as f32);
        self.text.push_str(character);
        self.previous = Some((x + width * scale, y));
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn extract(bytes: &[u8]) -> Result<String, OutputError> {
    Ok(extract_lines(bytes)?
        .into_iter()
        .map(|line| line.text)
        .collect::<Vec<_>>()
        .join("\n"))
}

pub(super) fn extract_lines(bytes: &[u8]) -> Result<Vec<domain::DocumentLine>, OutputError> {
    let mut doc = Document::load_mem(bytes)?;
    if doc.is_encrypted() {
        doc.decrypt("")?;
    }
    let mut output = TextOutput::default();
    pdf_extract::output_doc(&doc, &mut output)?;
    output.sizes.push(output.line_size);
    Ok(output
        .text
        .split('\n')
        .zip(output.sizes)
        .filter_map(|(text, size)| {
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            (!text.is_empty()).then_some(domain::DocumentLine {
                text,
                emphasized: false,
                font_size: Some(size),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_run_gaps_are_not_spaces_but_words_and_lines_survive() {
        let mut output = TextOutput::default();
        for (x, y, text) in [
            (0., 0., "A"),
            (6.5, 0., "da"),
            (21., 0., "Lovelace"),
            (0., -14., "Engineer"),
        ] {
            let trm = Transform::row_major(1., 0., 0., 1., x, y);
            output.output_character(&trm, 0.5, 0., 12., text).unwrap();
        }
        assert_eq!(output.text, "Ada Lovelace\nEngineer");
    }

    #[test]
    fn extracts_a_real_pdf_with_split_text_runs() {
        use pdf_extract::{Object, Stream, dictionary};
        let mut doc = Document::with_version("1.5");
        let pages = doc.new_object_id();
        let font = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        });
        // Helvetica 'A' ends at x=8.004. A 1.5pt adjustment is tracking,
        // not the word boundary inferred by the default extractor.
        let content = doc.add_object(Stream::new(
            dictionary! {},
            b"BT /F1 12 Tf 1 0 0 1 0 100 Tm (A) Tj 9.504 0 Td (da) Tj ET".to_vec(),
        ));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages,
            "MediaBox" => vec![0.into(), 0.into(), 300.into(), 300.into()],
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
            "Contents" => content,
        });
        doc.objects.insert(
            pages,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1,
            }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        doc.trailer.set("Root", catalog);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        assert_eq!(extract(&bytes).unwrap().trim(), "Ada");
        assert_eq!(extract_lines(&bytes).unwrap()[0].font_size, Some(12.));
    }

    #[test]
    fn initial_capitals_do_not_swallow_real_word_gaps() {
        for (initial, word) in [("A", "team"), ("I", "enjoy"), ("R", "programming")] {
            let mut output = TextOutput::default();
            for (x, text) in [(0., initial), (8.5, word)] {
                output
                    .output_character(
                        &Transform::row_major(1., 0., 0., 1., x, 0.),
                        0.5,
                        0.,
                        12.,
                        text,
                    )
                    .unwrap();
            }
            assert_eq!(output.text, format!("{initial} {word}"));
        }
    }

    #[test]
    fn explicit_spaces_and_page_boundaries_survive() {
        let mut output = TextOutput::default();
        let trm = Transform::identity();
        for text in ["A", " ", "B"] {
            output.output_character(&trm, 0.5, 0., 12., text).unwrap();
        }
        output
            .begin_page(
                2,
                &MediaBox {
                    llx: 0.,
                    lly: 0.,
                    urx: 100.,
                    ury: 100.,
                },
                None,
            )
            .unwrap();
        output.output_character(&trm, 0.5, 0., 12., "C").unwrap();
        assert_eq!(output.text, "A B\nC");
    }
}
