use std::process::Command;

fn decode(input: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rustorrent"))
        .args(["decode", input])
        .output()
        .expect("failed to run rustorrent binary");
    assert!(out.status.success(), "binary exited with {:?}", out.status);
    String::from_utf8(out.stdout)
        .expect("non-utf8 stdout")
        .trim_end()
        .to_string()
}

fn check(input: &str, expected: &str) {
    assert_eq!(decode(input), expected, "input: {input}");
}

fn byte_array(s: &str) -> String {
    let nums = s.as_bytes().iter().map(u8::to_string).collect::<Vec<_>>();
    format!("[{}]", nums.join(","))
}

#[test]
fn integers() {
    for (input, expected) in [
        ("i0e", "0"),
        ("i52e", "52"),
        ("i-3e", "-3"),
        ("i123456789e", "123456789"),
        ("i-123456789e", "-123456789"),
    ] {
        check(input, expected);
    }
}

#[test]
fn strings() {
    for (input, expected) in [
        ("5:hello", byte_array("hello")),
        ("5:world", byte_array("world")),
        ("0:", "[]".to_string()),
        ("11:hello world", byte_array("hello world")),
        ("11:hello:world", byte_array("hello:world")),
    ] {
        check(input, &expected);
    }
}

#[test]
fn lists() {
    for (input, expected) in [
        ("le", "[]".to_string()),
        ("l5:helloi52ee", format!("[{},52]", byte_array("hello"))),
        (
            "lli52e5:helloee",
            format!("[[52,{}]]", byte_array("hello")),
        ),
        ("lli4eei5ee", "[[4],5]".to_string()),
    ] {
        check(input, &expected);
    }
}

#[test]
fn dicts() {
    for (input, expected) in [
        ("de", "{}".to_string()),
        (
            "d3:foo5:hello5:helloi52ee",
            format!(r#"{{"foo":{},"hello":52}}"#, byte_array("hello")),
        ),
        (
            "d10:inner_dictd4:key16:value14:key2i42e8:list_keyl5:item15:item2i3eeee",
            format!(
                r#"{{"inner_dict":{{"key1":{},"key2":42,"list_key":[{},{},3]}}}}"#,
                byte_array("value1"),
                byte_array("item1"),
                byte_array("item2"),
            ),
        ),
    ] {
        check(input, &expected);
    }
}