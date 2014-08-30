#![feature(phase)]

extern crate regex;
#[phase(plugin)]
extern crate regex_macros;

use std::io::{
    File,
    IoResult
};
use std::os::args;
use std::io::BufferedReader;
use std::io::BufferedWriter;
use std::path::GenericPath;
use std::io::MemReader;
use std::io::MemWriter;

#[deriving(Show, PartialEq)]
enum LineType {
    WholeFile(String), // (filename)
    Section(String, String), // (filename, sectionname)
    Lines(String, uint, uint) // (filename, startline, endline)
}

enum MdError {
    IoProblem(IoError),
    NonMatchingCode(String, uint),
    SectionNotFound(String, uint),
    InvalidLineChunk(String, uint, uint),
    FileTooSmall(String, uint)
}

fn modify_path(mkdev_path: &str) -> String {
    if mkdev_path.ends_with(".md.dev") {
        return mkdev_path.slice_to(mkdev_path.len() - 4).to_string();
    } else {
        let mut buf = mkdev_path.to_string();
        buf.push_str(".md");
        return buf;
    }
}


fn detect_type(line: &str) -> Option<LineType> {
    let file = regex!(r"\^code\( *([^, ]+) *\)");
    let section = regex!(r"\^code\( *([^, ]+) *, *([a-zA-Z]+) *\)");
    let lines = regex!(r"\^code\( *([^, ]+) *, *([0-9]+) *, *([0-9]+) *\)");

    if file.is_match(line) {
        let capture = file.captures(line).unwrap();
        Some(WholeFile(capture.at(1).to_string()))
    } else if section.is_match(line) {
        let capture = section.captures(line).unwrap();
        Some(Section(capture.at(1).to_string(), capture.at(2).to_string()))
    } else if lines.is_match(line) {
        let capture = lines.captures(line).unwrap();
        let (start, end) = (from_str(capture.at(2)), from_str(capture.at(3)));
        match (start, end) {
            (Some(s), Some(e)) => Some(Lines(capture.at(1).to_string(), s, e)),
            _ => None
        }
    } else {
        None
    }
}

fn rewrite<R: Reader, W: Writer>
(linetype: LineType, fetch: |&str| -> BufferedReader<R>, out_buffer: &mut BufferedWriter<W>) -> IoResult<()> {
    let file = match linetype {
        WholeFile(ref s) => s,
        Section(ref s, _) => s,
        Lines(ref s, _, _) => s,
    }.as_slice();

    let mut reader = fetch(file);

    match linetype {
        WholeFile(_) => {
            for line in reader.lines() {
                try!(out_buffer.write_str(try!(line).as_slice()));
            }
            out_buffer.write_line("");
            Ok(())
        }
        Section(_, section_name) => {
            let mut scanning = false;
            for line in reader.lines() {
                let line = try!(line);
                let trimmed = line.as_slice().trim_left_chars(' ');
                let prelude = "// section ";
                if trimmed.starts_with(prelude) {
                    let name = trimmed
                        .slice_from(prelude.len())
                        .trim_chars(' ')
                        .trim_chars('\n');
                    if scanning {
                        break;
                    } else {
                        if name == section_name.as_slice() {
                            scanning = true;
                        }
                    }
                } else if scanning {
                    out_buffer.write_line(line.as_slice().trim_right_chars('\n'));
                }
            }
            Ok(())
        }
        Lines(_, start, end) => {
            for line in reader.lines().skip(start).take(end - start + 1) {
                try!(out_buffer.write_line(try!(line).as_slice().trim_right_chars('\n')));
            }
            Ok(())
        }
    }
}

fn process_file<R: Reader, W: Writer>
(in_buffer: BufferedReader<R>, out_buffer: BufferedWriter<W>, fetch: |&str| -> BufferedReader<R>) -> IoResult<()> {
    let mut in_buffer = in_buffer;
    let mut out_buffer = out_buffer;
    for line in in_buffer.lines() {
        let line = try!(line);
        let line = line.as_slice();
        if line.starts_with("^code") {
            match detect_type(line) {
                Some(typ) => {
                    try!(out_buffer.write_line("```rust"));
                    try!(rewrite(typ, |a| fetch(a), &mut out_buffer));
                    try!(out_buffer.write_line("```"));
                }
                None => {

                }
            }
        } else {
            try!(out_buffer.write_line(line));
        }
    }
    Ok(())
}

fn main() {
    for file in args().iter() {
        let file = file.as_slice();
        //process_file(file, modify_path(file).as_slice());
    }
}



#[test]
fn test_modify_path() {
    assert_eq!(modify_path("foo.md.dev"), "foo.md".to_string());
    assert_eq!(modify_path("foo"), "foo.md".to_string());
}

#[test]
fn test_detect_type() {
    assert_eq!(
        detect_type("^code(abc.rs)"),
        Some(WholeFile("abc.rs".to_string())));
    assert_eq!(
        detect_type("^code(abc.rs,sec)"),
        Some(Section("abc.rs".to_string(), "sec".to_string())));
    assert_eq!(
        detect_type("^code(abc.rs,0,10)"),
        Some(Lines("abc.rs".to_string(), 0, 10)));
    assert_eq!(
        detect_type("^code(  abc.rs    )"),
        Some(WholeFile("abc.rs".to_string())));
    assert_eq!(
        detect_type("^code(    abc.rs  ,  sec   )"),
        Some(Section("abc.rs".to_string(), "sec".to_string())));
}

#[test]
fn test_rewrite() {
    fn run_rewrite<S: Str>(lt: LineType, provided: Vec<S>) -> String {
        let string_form = provided.connect("\n");
        let mut input = vec![];
        input.push_all(string_form.as_bytes());
        let c = || BufferedReader::new(MemReader::new(input.clone()));

        let output = MemWriter::new();
        let mut out_buf = BufferedWriter::new(output);
        rewrite(lt, |_| c(), &mut out_buf);
        String::from_utf8(out_buf.unwrap().unwrap()).unwrap()
    }
    assert_eq!(run_rewrite(WholeFile("a".to_string()), vec!["foo"]),
               "foo\n".to_string());
    assert_eq!(run_rewrite(WholeFile("a".to_string()),
                   vec!["foo", "bar", "baz"]),
               "foo\nbar\nbaz\n".to_string());

    assert_eq!(run_rewrite(Section("a".to_string(), "f".to_string()),
                    vec!["abc", "// section f", "foo", "bar"]),
               "foo\nbar\n".to_string());
    assert_eq!(run_rewrite(Section("a".to_string(), "f".to_string()),
                    vec!["abc", "// section f", "foo",
                         "bar", "// section baz", "go"]),
               "foo\nbar\n".to_string());
    assert_eq!(run_rewrite(Lines("a".to_string(), 1, 3),
                    vec!["abc", "bar", "foo",
                         "bar", "back", "go"]),
               "bar\nfoo\nbar\n".to_string());
}
