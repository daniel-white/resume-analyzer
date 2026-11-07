//! Greenline, Levr, Workdaye, and Ash on the two fixture resumes.
//! Factor labels and signs are the assertion. The headline score is not.

use std::io::{Cursor, Write};

use domain::{DOCX, LayoutConfidence, ResumeFile};
use plugins::{Ash, Greenline, Levr, Workdaye};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

fn stored() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
}

fn docx(header: &str, body: &str) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer.start_file("word/header1.xml", stored()).unwrap();
    writer.write_all(header.as_bytes()).unwrap();
    writer.start_file("word/document.xml", stored()).unwrap();
    writer.write_all(body.as_bytes()).unwrap();
    writer.finish().unwrap().into_inner()
}

/// Contact in the header, skills in a table, a text box, two columns, an
/// image, and a date that is not MM/YYYY.
fn designed() -> Vec<u8> {
    docx(
        "<w:t>Ada Lovelace</w:t><w:t>ada@example.com</w:t>",
        concat!(
            r#"<w:cols w:num="2"/>"#,
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Experience</w:t></w:r></w:p>"#,
            "<w:p><w:r><w:t>2019</w:t></w:r></w:p>",
            "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Rust</w:t></w:r></w:p></w:tc></w:tr></w:tbl>",
            "<w:txbxContent><w:p><w:r><w:t>sidebar</w:t></w:r></w:p></w:txbxContent>",
            "<w:drawing><a:blip/></w:drawing>",
        ),
    )
}

/// One column, contact in the body, MM/YYYY, standard headings, bullets, and
/// no table, text box, or image.
fn plain() -> Vec<u8> {
    docx(
        "",
        concat!(
            "<w:p><w:r><w:t>Ada Lovelace</w:t></w:r></w:p>",
            "<w:p><w:r><w:t>ada@example.com</w:t></w:r></w:p>",
            "<w:p><w:r><w:t>01/2020</w:t></w:r></w:p>",
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Experience</w:t></w:r></w:p>"#,
            "<w:p><w:pPr><w:numPr/></w:pPr><w:r><w:t>Shipped a parser.</w:t></w:r></w:p>",
            "<w:p><w:pPr><w:numPr/></w:pPr><w:r><w:t>Shipped another.</w:t></w:r></w:p>",
        ),
    )
}

fn labels(file: &[u8], plugin: &dyn domain::Ats) -> Vec<(String, i32)> {
    let read = plugin.read(&ResumeFile {
        media_type: DOCX,
        bytes: file,
    });
    assert_eq!(
        read.parse.layout_confidence,
        LayoutConfidence::Structural,
        "{} reads docx markup as structure",
        plugin.id()
    );
    read.verdict
        .factors
        .into_iter()
        .map(|factor| (factor.label, factor.delta.signum()))
        .collect()
}

#[test]
fn greenline_levr_workdaye_and_ash_disagree_on_the_designed_fixture() {
    let file = designed();
    let greenline = labels(&file, &Greenline::new("secret"));
    let levr = labels(&file, &Levr::new("secret"));
    let workdaye = labels(&file, &Workdaye::new("secret"));
    let ash = labels(&file, &Ash::new("secret"));

    let lists = [&greenline, &levr, &workdaye, &ash];
    for index in 0..lists.len() {
        for other in (index + 1)..lists.len() {
            assert_ne!(lists[index], lists[other], "designed factor lists tied");
        }
    }

    assert_eq!(
        greenline,
        vec![
            ("contact in header".into(), -1),
            ("table".into(), -1),
            ("text box".into(), -1),
            ("two columns".into(), -1),
            ("standard headings".into(), 1),
        ]
    );
    assert_eq!(levr, vec![("skills not normalized".into(), -1)]);
    assert_eq!(
        workdaye,
        vec![
            ("contact in header".into(), -1),
            ("table".into(), -1),
            ("text box".into(), -1),
            ("two columns".into(), -1),
            ("dates not MM/YYYY".into(), -1),
            ("standard headings".into(), 1),
        ]
    );
    assert_eq!(
        ash,
        vec![
            ("email kept".into(), 1),
            ("skills normalized".into(), 1),
            ("image".into(), -1),
            ("standard headings".into(), 1),
        ]
    );
}

#[test]
fn the_fleet_agrees_on_the_plain_fixture() {
    let file = plain();
    let fleet: Vec<Box<dyn domain::Ats>> = vec![
        Box::new(Greenline::new("secret")),
        Box::new(Levr::new("secret")),
        Box::new(Workdaye::new("secret")),
        Box::new(Ash::new("secret")),
    ];

    for plugin in &fleet {
        let read = plugin.read(&ResumeFile {
            media_type: DOCX,
            bytes: &file,
        });
        assert!(
            read.verdict.dropped.is_empty(),
            "{} dropped {:?} on the plain resume",
            plugin.id(),
            read.verdict.dropped
        );
        assert!(
            read.verdict.factors.iter().all(|factor| factor.delta > 0),
            "{} punished the plain resume: {:?}",
            plugin.id(),
            read.verdict
                .factors
                .iter()
                .map(|factor| factor.label.as_str())
                .collect::<Vec<_>>()
        );
        assert!(
            plugin.subtitle().contains("Simulated"),
            "{} subtitle must say the system is simulated",
            plugin.id()
        );
    }
}
