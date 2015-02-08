use std::old_io::MemReader;
use std::old_io::MemWriter;
use std::old_io::BufferedReader;
use std::old_io::BufferedWriter;

use super::*;

#[allow(unused_must_use)]

#[test]
fn test_detect_type() {
    assert_eq!(
        detect_type("^code(abc.rs)"),
        Some(LineType::WholeFile("abc.rs".to_string())));
    assert_eq!(
        detect_type("^code(abc.rs,sec)"),
        Some(LineType::Section("abc.rs".to_string(), "sec".to_string())));
    assert_eq!(
        detect_type("^code(abc.rs,0,10)"),
        Some(LineType::Lines("abc.rs".to_string(), 0, 10)));
    assert_eq!(
        detect_type("^code(  abc.rs    )"),
        Some(LineType::WholeFile("abc.rs".to_string())));
    assert_eq!(
        detect_type("^code(    abc.rs  ,  sec   )"),
        Some(LineType::Section("abc.rs".to_string(), "sec".to_string())));
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
        if rewrite(lt, |_| c(), &mut out_buf).is_err() {
            panic!();
        }
        String::from_utf8(out_buf.into_inner().into_inner()).unwrap()
    }
    assert_eq!(run_rewrite(LineType::WholeFile("a".to_string()), vec!["foo"]),
               "foo\n".to_string());
    assert_eq!(run_rewrite(LineType::WholeFile("a".to_string()),
                   vec!["foo", "bar", "baz"]),
               "foo\nbar\nbaz\n".to_string());

    assert_eq!(run_rewrite(LineType::Section("a".to_string(), "f".to_string()),
                    vec!["abc", "// section f", "foo", "bar"]),
               "foo\nbar\n".to_string());
    assert_eq!(run_rewrite(LineType::Section("a".to_string(), "f".to_string()),
                    vec!["abc", "// section f", "foo",
                         "bar", "// section baz", "go"]),
               "foo\nbar\n".to_string());
    assert_eq!(run_rewrite(LineType::Lines("a".to_string(), 1, 3),
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
        String::from_utf8(writer.into_inner().into_inner()).unwrap()
    }
    fn run_test(lines: Vec<&'static str>) -> String {
        let mut writer = BufferedWriter::new(MemWriter::new());
        let mut reader = str_to_input_buffer(lines);
        if process_file(&mut reader, &mut writer, grab_files).is_err() {
            panic!();
        }
        out_buffer_to_string(writer)
    }
    assert_eq!(run_test(
        vec![ "a", "b", "^code(a.rs, a)" ]),
       "a\nb\n```rust\nblue whale\nfoo\n```\n".to_string());

    assert_eq!(run_test(
        vec![ "a", "b", "^code(a.rs, a)", "c" ]),
       "a\nb\n```rust\nblue whale\nfoo\n```\nc\n".to_string());

    assert_eq!(run_test(
        vec![ "a", "b", "^code(b.rs)", "c" ]),
       "a\nb\n```rust\nfizz\nbuzzl\nbar\n```\nc\n".to_string());

    assert_eq!(run_test(
        vec![ "a", "b", "^code(c.rs, 1, 2)", "c" ]),
       "a\nb\n```rust\nit's a trap\nbar\n```\nc\n".to_string());
}
