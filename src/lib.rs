#![feature(phase)]

extern crate regex;
#[phase(plugin)]
extern crate regex_macros;
#[phase(plugin)]
extern crate try_or;

use std::io::{
    File,
    IoError
};
use std::io::BufferedReader;
use std::io::BufferedWriter;
use std::path::GenericPath;
#[cfg(test)]
use std::io::MemReader;
#[cfg(test)]
use std::io::MemWriter;

#[deriving(Show, PartialEq)]
enum LineType {
    WholeFile(String), // (filename)
    Section(String, String), // (filename, sectionname)
    Lines(String, uint, uint) // (filename, startline, endline)
}

#[deriving(PartialEq, Eq, Show)]
pub enum MdError {
    OpenReadError(IoError),
    OpenWriteError(IoError),
    SourceError(IoError),
    ImportError(IoError),
    OutputError(IoError),
    NonMatchingCode(String),
    SectionNotFound(String, uint),
    InvalidLineChunk(String),
    FileTooSmall(String, uint)
}

pub type MdResult<A> = Result<A, MdError>;

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
(linetype: LineType, fetch: |&str| -> MdResult<BufferedReader<R>>,
out_buffer: &mut BufferedWriter<W>) -> MdResult<()> {
    let file = match linetype {
        WholeFile(ref s) => s,
        Section(ref s, _) => s,
        Lines(ref s, _, _) => s,
    }.as_slice();

    let mut reader = try_or!(fetch(file));

    match linetype {
        WholeFile(_) => {
            for line in reader.lines() {
                let line = try_or!(line, ImportError);
                let line = line.as_slice();
                try_or!(out_buffer.write_str(line), OutputError);
            }
            try_or!(out_buffer.write_line(""), OutputError);
            Ok(())
        }
        Section(_, section_name) => {
            let mut scanning = false;
            for line in reader.lines() {
                let line = try_or!(line, ImportError);
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
                    let line = line.as_slice().trim_right_chars('\n');
                    try_or!(out_buffer.write_line(line), OutputError);
                }
            }
            Ok(())
        }
        Lines(_, start, end) => {
            for line in reader.lines().skip(start).take(end - start + 1) {
                let line = try_or!(line, ImportError);
                let line = line.as_slice().trim_right_chars('\n');
                try_or!(out_buffer.write_line(line), OutputError);
            }
            Ok(())
        }
    }
}

pub fn process_file<R: Reader, W: Writer>
(in_buffer: &mut BufferedReader<R>, out_buffer: &mut BufferedWriter<W>,
fetch: |&str| -> MdResult<BufferedReader<R>>) -> MdResult<()> {
    let in_buffer = in_buffer;
    let out_buffer = out_buffer;
    for line in in_buffer.lines() {
        let line = try_or!(line, SourceError);
        let line = line.as_slice();
        if line.starts_with("^code") {
            match detect_type(line) {
                Some(typ) => {
                    try_or!(out_buffer.write_line("```rust"), OutputError);
                    try_or!(rewrite(typ, |a| fetch(a), out_buffer));
                    try_or!(out_buffer.write_line("```"), OutputError);
                }
                None => {

                }
            }
        } else {
            try_or!(out_buffer.write_line(line.trim_right_chars('\n')), OutputError);
        }
    }
    Ok(())
}

pub fn transform_file(source: &str) -> MdResult<()> {
    let out_name = {
        let mut base;
        if source.ends_with(".dev.md") {
            base = String::from_str(source.slice_to(source.len() - 7));
        } else {
            base = String::from_str(source);
        }
        base.push_str(".md");
        base
    };
    let in_path = Path::new(source);
    let out_path = Path::new(out_name);
    let mut relative_path = in_path.clone();
    relative_path.pop();

    let read_file = try_or!(File::open(&in_path), OpenReadError);
    let write_file = try_or!(File::create(&out_path), OpenWriteError);

    let mut read_buffer = BufferedReader::new(read_file);
    let mut write_buffer = BufferedWriter::new(write_file);

    process_file(&mut read_buffer, &mut write_buffer, |extra| {
        let mut temp = relative_path.clone();
        temp.push(extra);
        let source_file = try_or!(File::open(&temp), OpenReadError);
        Ok(BufferedReader::new(source_file))
    })
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
        let c = || Ok(BufferedReader::new(MemReader::new(input.clone())));

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

#[test]
fn test_process_files() {
    fn str_to_input_buffer<S: Str>(lines: Vec<S>) -> BufferedReader<MemReader> {
        let string_form = lines.connect("\n");
        let mut input = vec![];
        input.push_all(string_form.as_bytes());
        BufferedReader::new(MemReader::new(input.clone()))
    }
    fn grab_files(filename: &str) -> MdResult<BufferedReader<MemReader>> {
        Ok(match filename {
            "a.rs" => str_to_input_buffer(vec!["ars", "// section a", "blue whale", "foo"]),
            "b.rs" => str_to_input_buffer(vec!["fizz", "buzzl", "bar"]),
            "c.rs" => str_to_input_buffer(vec!["ack", "it's a trap", "bar"]),
            _ => str_to_input_buffer(vec!["foo"])
        })
    }
    fn out_buffer_to_string(writer: BufferedWriter<MemWriter>) -> String {
        String::from_utf8(writer.unwrap().unwrap()).unwrap()
    }
    fn run_test(lines: Vec<&'static str>) -> String {
        let mut writer = BufferedWriter::new(MemWriter::new());
        let mut reader = str_to_input_buffer(lines);
        process_file(&mut reader, &mut writer, grab_files);
        out_buffer_to_string(writer)
    }
    assert_eq!(run_test(
        vec![ "a", "b", "^code(a.rs, a)" ]),
       "a\nb\n```rust\nblue whale\nfoo\n```\n".to_string())

    assert_eq!(run_test(
        vec![ "a", "b", "^code(a.rs, a)", "c" ]),
       "a\nb\n```rust\nblue whale\nfoo\n```\nc\n".to_string())

    assert_eq!(run_test(
        vec![ "a", "b", "^code(b.rs)", "c" ]),
       "a\nb\n```rust\nfizz\nbuzzl\nbar\n```\nc\n".to_string())

    assert_eq!(run_test(
        vec![ "a", "b", "^code(c.rs, 1, 2)", "c" ]),
       "a\nb\n```rust\nit's a trap\nbar\n```\nc\n".to_string())
}
