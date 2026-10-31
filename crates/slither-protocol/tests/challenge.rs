mod common;
use slither_protocol::challenge::*;
const SOURCE: &[u8] = b"var a = \"_k/T)\";a+=\"dr'Ay(^|``J9l@Bs'L{]JX\";for (var i=0;i<27;i++){idba[i]=a.charCodeAt(i);}";
#[test]
fn algorithms_have_separate_independent_golden_replies() {
    let encoded = common::encode_source(SOURCE);
    for grammar in [Grammar::ExactTemplate, Grammar::TokenWhitespace] {
        assert_eq!(
            stock(&encoded, grammar).unwrap(),
            *b"LzLIOvqUYaFMrAJWShKPlPEiOBC"
        );
        assert_eq!(
            literal_27(&encoded, grammar).unwrap(),
            *b"_k/T)dr'Ay(^|``J9l@Bs'L{]JX"
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
    let source = b" \tvar a=\"_k/T)\" ; a += \"dr'Ay(^|``J9l@Bs'L{]JX\" ;\nfor(var i=0;i<27;i++){idba[i]=a.charCodeAt(i);} \r";
    let encoded = common::encode_source(source);
    assert_eq!(
        stock(&encoded, Grammar::TokenWhitespace).unwrap(),
        *b"LzLIOvqUYaFMrAJWShKPlPEiOBC"
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
