mod common;
use slither_protocol::challenge::*;
// Authored synthetic literals, including punctuation with signed remainder.
const SOURCE: &[u8] = b"var a = \" !Z_a\";a+=\"0123456789ABCdefghij!?\";for (var i=0;i<27;i++){idba[i]=a.charCodeAt(i);}";
#[test]
fn exact_literal_errors_point_to_the_offending_encoded_byte() {
    // First/last positions in both literals, including the second-literal start
    // that previously reported the first literal's unscaled source offset.
    for at in [9, 13, 20, 41] {
        for invalid in [b'"', b'\\', 0, 127, 255] {
            let mut source = SOURCE.to_vec();
            source[at] = invalid;
            let error =
                literal_27(&common::encode_source(&source), Grammar::ExactTemplate).unwrap_err();
            assert_eq!(
                (error.kind, error.opcode, error.offset),
                (slither_protocol::ErrorKind::Literal, Some(b'6'), at * 2)
            );
        }
    }
}
#[test]
fn algorithms_have_separate_independent_golden_replies() {
    let encoded = common::encode_source(SOURCE);
    for grammar in [Grammar::ExactTemplate, Grammar::TokenWhitespace] {
        assert_eq!(
            stock(&encoded, grammar).unwrap(),
            *b"647?I6*9/@81+@<@+1X`OYJVIA)"
        );
        assert_eq!(
            literal_27(&encoded, grammar).unwrap(),
            *b" !Z_a0123456789ABCdefghij!?"
        );
        // The step-2 array is independently pinned by the browser admission fixture.
        let host = common::encode_source(b"var a = \"Slith\";a+=\"erSimLocalHostChalleng\";for (var i=0;i<27;i++){idba[i]=a.charCodeAt(i);}");
        assert_eq!(
            vlither_27(&host, grammar).unwrap(),
            *b"NakfounHrfSimobKaupTcerekzh"
        );
    }
    let legacy = common::encode_source(
        b"var a='ABCDEFGHIJKLMNOPQRSTUVWX';for(var i=0;i<24;i++)idba[i]=a.charCodeAt(i);",
    );
    assert_eq!(
        ntl_legacy_24(&legacy).unwrap(),
        *b"CGLRYGPZKWJXMCTLEYTPMKJJ"
    );
}
#[test]
fn whitespace_is_only_admitted_by_token_grammar() {
    let source = b" \tvar a=\" !Z_a\" ; a += \"0123456789ABCdefghij!?\" ;\nfor(var i=0;i<27;i++){idba[i]=a.charCodeAt(i);} \r";
    let encoded = common::encode_source(source);
    assert_eq!(
        stock(&encoded, Grammar::TokenWhitespace).unwrap(),
        *b"647?I6*9/@81+@<@+1X`OYJVIA)"
    );
    assert!(stock(&encoded, Grammar::ExactTemplate).is_err());
    for bad in [
        b"while(true){}".as_slice(),
        b"idba.constructor.constructor('return process')()",
        b"varx a=0",
    ] {
        for grammar in [Grammar::ExactTemplate, Grammar::TokenWhitespace] {
            assert!(stock(&common::encode_source(bad), grammar).is_err());
        }
    }
    for suffix in [b";idba.fill(65)".as_slice(), b"//tail"] {
        let mut source = SOURCE.to_vec();
        source.extend(suffix);
        assert!(stock(&common::encode_source(&source), Grammar::TokenWhitespace).is_err());
    }
    for replacement in [b'"', b'\\', 0, 127, 255] {
        let mut source = SOURCE.to_vec();
        source[9] = replacement;
        for grammar in [Grammar::ExactTemplate, Grammar::TokenWhitespace] {
            assert!(stock(&common::encode_source(&source), grammar).is_err());
        }
    }
    for bad in [
        vec![],
        vec![b'e'],
        vec![b'!'; 2],
        vec![b'z'; 2],
        vec![b'a'; 514],
    ] {
        assert!(stock(&bad, Grammar::TokenWhitespace).is_err());
        assert!(ntl_legacy_24(&bad).is_err());
    }
}

#[test]
fn general_copy_loop_has_independent_synthetic_golden_replies() {
    // Authored single-quoted 24-letter source, never captured server bytes.
    let legacy = common::encode_source(
        b"var a='ABCDEFGHIJKLMNOPQRSTUVWX';for(var i=0;i<24;i++)idba[i]=a.charCodeAt(i);",
    );
    let reply = copy_loop(&legacy).unwrap();
    assert_eq!(reply.as_bytes(), b"DIOVDMWHTGUJZQIBVQMJHGGH");
    assert_eq!(reply.len(), 24);
    assert!(!reply.is_empty());
    assert_ne!(reply.as_bytes(), ntl_legacy_24(&legacy).unwrap());
    assert!(stock(&legacy, Grammar::TokenWhitespace).is_err());
    for (source,expected) in [
        (&b"var a='A';a+=\"b\";a+='C';for(var i=0;i<3;i++){idba[i]=a.charCodeAt(i);}"[..],&b"DiO"[..]),
        (&b" \tvar a = ' !' ; a += \"aZ~\" ;\nfor ( var i = 0 ; i < 5 ; i ++ ) idba [ i ] = a . charCodeAt ( i ) ; \r"[..],&b"64X;H"[..]),
        (&b"var a='a';for(var i=0;i<1;i++)idba[i]=a.charCodeAt(i);"[..],&b"d"[..]),
        (&b"var a='\"';a+=\"'\";for(var i=0;i<2;i++){idba[i]=a.charCodeAt(i);}"[..],&b":>"[..]),
    ] {
        assert_eq!(copy_loop(&common::encode_source(source)).unwrap().as_bytes(),expected);
    }
    assert_eq!(
        copy_loop(&common::encode_source(SOURCE))
            .unwrap()
            .as_bytes(),
        b"647?I6*9/@81+@<@+1X`OYJVIA)"
    );
    let uppercase = legacy
        .iter()
        .map(u8::to_ascii_uppercase)
        .collect::<Vec<_>>();
    assert_eq!(copy_loop(&uppercase).unwrap(), reply);
}

#[test]
fn general_copy_loop_rejects_other_programs_and_mismatched_counts() {
    let source = "var a='ABC';for(var i=0;i<3;i++)idba[i]=a.charCodeAt(i);";
    for bad in [
        source.replace("'ABC'", "''"),
        source.replace("'ABC'", "'A\\B'"),
        source.replace("'ABC'", "'A\nB'"),
        source.replace("'ABC'", "'A\x7fB'"),
        source.replace("'ABC'", "'AéB'"),
        source.replace("'ABC'", "'ABC\""),
        source.replace("'ABC'", "'A'+'BC'"),
        source.replace("'ABC'", "String('ABC')"),
        source.replace("'ABC'", "'A';a+='';a+='BC'"),
        source.replace("var a", "vara"),
        source.replace("var a", "let a"),
        source.replace("i=0", "i=1"),
        source.replace("i<3", "i<=3"),
        source.replace("i<3", "i<0"),
        source.replace("i<3", "i<03"),
        source.replace("i<3", "i<2"),
        source.replace("i<3", "i<4"),
        source.replace("i<3", "i<999"),
        source.replace("i<3", "i<1000"),
        source.replace("i<3", "i<+3"),
        source.replace("i<3", "i<3.0"),
        source.replace("i<3", "i<3n"),
        source.replace("i<3", "i<3e0"),
        source.replace("i<3", "i<0x3"),
        source.replace("i<3", "i<'3'"),
        source.replace("i++", "++i"),
        source.replace("i++", "i+=1"),
        source.replace("idba[i]", "idba[0]"),
        source.replace("charCodeAt(i)", "charCodeAt(0)"),
        source.replace("charCodeAt", "charCodeAtX"),
        source.replace("idba[i]=", "{idba[i]="),
        source.replace("idba[i]=", "{}idba[i]="),
        source.replace("for(", "/*comment*/for("),
        format!("{source};"),
        format!("{source}idba[0]=65;"),
        format!("{source}for(var i=0;i<3;i++)idba[i]=a.charCodeAt(i);"),
    ] {
        assert!(
            copy_loop(&common::encode_source(bad.as_bytes())).is_err(),
            "{bad:?}"
        );
    }
    for bad in [
        vec![],
        vec![b'e'],
        vec![b'!'; 2],
        vec![b'z'; 2],
        vec![b'a'; 514],
    ] {
        assert!(copy_loop(&bad).is_err());
    }
}

#[test]
fn general_copy_loop_bounds_length_and_reports_encoded_payload_offsets() {
    let prefix = "var a='";
    let suffix = "';for(var i=0;i<199;i++)idba[i]=a.charCodeAt(i);";
    // Fill the existing 256-byte source bound, including a three-digit count.
    let text = format!(
        "{prefix}{}{suffix}",
        "A".repeat(MAX_SOURCE_BYTES - prefix.len() - suffix.len())
    );
    let text = text.replace(
        "199",
        &(MAX_SOURCE_BYTES - prefix.len() - suffix.len()).to_string(),
    );
    assert_eq!(text.len(), MAX_SOURCE_BYTES);
    let encoded = common::encode_source(text.as_bytes());
    let reply = copy_loop(&encoded).unwrap();
    assert_eq!(reply.len(), MAX_SOURCE_BYTES - prefix.len() - suffix.len());
    assert!(reply.as_bytes().iter().all(|&b| b.is_ascii_uppercase()));
    let mut too_long = text.into_bytes();
    too_long.push(b' ');
    assert_eq!(
        copy_loop(&common::encode_source(&too_long))
            .unwrap_err()
            .kind,
        slither_protocol::ErrorKind::InvalidLength
    );
    for (source, at, kind) in [
        (
            &b"var a='A\\';"[..],
            8,
            slither_protocol::ErrorKind::Literal,
        ),
        (&b"var a='';"[..], 7, slither_protocol::ErrorKind::Literal),
        (
            &b"var a='A';for(var i=0;i<01;i++)idba[i]=a.charCodeAt(i);"[..],
            24,
            slither_protocol::ErrorKind::Syntax,
        ),
    ] {
        let e = copy_loop(&common::encode_source(source)).unwrap_err();
        assert_eq!((e.kind, e.opcode, e.offset), (kind, Some(b'6'), at * 2));
    }
    let mut invalid =
        common::encode_source(b"var a='A';for(var i=0;i<1;i++)idba[i]=a.charCodeAt(i);");
    invalid[17] = b'!';
    assert_eq!(copy_loop(&invalid).unwrap_err().offset, 17);
    // Every truncated prefix is rejected without indexing beyond either buffer.
    for n in 0..encoded.len() {
        assert!(copy_loop(&encoded[..n]).is_err());
    }
}
