//! Le découpage du flux TCP de Rocket League.
//!
//! Le jeu n'envoie AUCUN délimiteur, et ses objets font parfois des dizaines de
//! kilo-octets. On compte donc les accolades, en ignorant celles qui vivent
//! dans une chaîne et en respectant les échappements. Les deux implémentations
//! de `ke-rl-tracker` font exactement cela : c'est du vécu, pas de la prudence.

use serde_json::Value;

/// Prend le premier objet JSON complet du tampon.
///
/// Rend l'objet et ce qui reste, ou rien s'il faut lire davantage.
pub fn take_frame(buf: &[u8]) -> Option<(&[u8], &[u8])> {
    let start = buf.iter().position(|&b| b == b'{')?;

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for i in start..buf.len() {
        let b = buf[i];
        if escaped {
            escaped = false;
            continue;
        }
        if in_string {
            match b {
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&buf[start..=i], &buf[i + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

/// Décode une enveloppe `{"Event": "...", "Data": {...}}`.
///
/// `Data` arrive tantôt comme objet, tantôt comme chaîne contenant du JSON, et
/// parfois pas du tout. Les trois cas sont normaux.
pub fn decode(frame: &[u8]) -> Option<(String, Value)> {
    let v: Value = serde_json::from_slice(frame).ok()?;
    let name = v.get("Event")?.as_str()?.to_string();

    let data = match v.get("Data") {
        None | Some(Value::Null) => Value::Object(Default::default()),
        Some(Value::String(s)) => {
            serde_json::from_str(s).unwrap_or_else(|_| Value::Object(Default::default()))
        }
        Some(other) => other.clone(),
    };
    Some((name, data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_whole_object_is_taken() {
        let (obj, rest) = take_frame(br#"{"a":1}tail"#).unwrap();
        assert_eq!(obj, br#"{"a":1}"#);
        assert_eq!(rest, b"tail");
    }

    #[test]
    fn nothing_is_taken_from_an_incomplete_object() {
        assert!(take_frame(br#"{"a":"#).is_none());
    }

    #[test]
    fn nested_objects_are_counted() {
        let src = br#"{"a":{"b":{"c":1}}}"#;
        let (obj, rest) = take_frame(src).unwrap();
        assert_eq!(obj, src);
        assert!(rest.is_empty());
    }

    #[test]
    fn a_brace_inside_a_string_does_not_count() {
        let src = br#"{"a":"}{"}"#;
        let (obj, rest) = take_frame(src).unwrap();
        assert_eq!(obj, src);
        assert!(rest.is_empty());
    }

    #[test]
    fn an_escaped_quote_does_not_close_the_string() {
        // Sans cette règle, le compteur croirait la chaîne finie et couperait
        // l'objet en plein milieu.
        let src = br#"{"a":"say \" }"}"#;
        let (obj, rest) = take_frame(src).unwrap();
        assert_eq!(obj, src);
        assert!(rest.is_empty());
    }

    #[test]
    fn an_escaped_backslash_does_not_escape_the_next_quote() {
        let src = br#"{"a":"c:\\"}"#;
        let (obj, rest) = take_frame(src).unwrap();
        assert_eq!(obj, src);
        assert!(rest.is_empty());
    }

    #[test]
    fn leading_noise_before_the_first_brace_is_skipped() {
        let (obj, _) = take_frame(br#"  \r\n{"a":1}"#).unwrap();
        assert_eq!(obj, br#"{"a":1}"#);
    }

    #[test]
    fn two_objects_are_taken_one_after_the_other() {
        let mut buf: Vec<u8> = br#"{"a":1}{"b":2}"#.to_vec();
        let (first, rest) = take_frame(&buf).unwrap();
        assert_eq!(first, br#"{"a":1}"#);
        buf = rest.to_vec();
        let (second, rest) = take_frame(&buf).unwrap();
        assert_eq!(second, br#"{"b":2}"#);
        assert!(rest.is_empty());
    }

    #[test]
    fn an_object_split_across_two_reads_is_taken_once_whole() {
        // C'est le cas normal : la socket rend ce qu'elle a, pas ce qu'on veut.
        let mut buf: Vec<u8> = br#"{"Event":"Update"#.to_vec();
        assert!(take_frame(&buf).is_none());
        buf.extend_from_slice(br#"State","Data":{}}"#);
        let (obj, rest) = take_frame(&buf).unwrap();
        assert_eq!(obj, br#"{"Event":"UpdateState","Data":{}}"#);
        assert!(rest.is_empty());
    }

    #[test]
    fn an_envelope_with_an_object_data_is_decoded() {
        let (name, data) = decode(br#"{"Event":"GoalScored","Data":{"GoalSpeed":97.5}}"#).unwrap();
        assert_eq!(name, "GoalScored");
        assert_eq!(data["GoalSpeed"], 97.5);
    }

    #[test]
    fn an_envelope_with_a_string_data_is_decoded_too() {
        // Le jeu encode parfois Data comme une CHAÎNE contenant du JSON. Les
        // deux implémentations d'origine gèrent ce cas, donc il arrive vraiment.
        let (name, data) =
            decode(br#"{"Event":"GoalScored","Data":"{\"GoalSpeed\":97.5}"}"#).unwrap();
        assert_eq!(name, "GoalScored");
        assert_eq!(data["GoalSpeed"], 97.5);
    }

    #[test]
    fn an_envelope_without_data_decodes_to_an_empty_object() {
        let (name, data) = decode(br#"{"Event":"MatchDestroyed"}"#).unwrap();
        assert_eq!(name, "MatchDestroyed");
        assert!(data.as_object().unwrap().is_empty());
    }

    #[test]
    fn garbage_decodes_to_nothing() {
        assert!(decode(b"not json").is_none());
        assert!(
            decode(br#"{"Data":{}}"#).is_none(),
            "sans Event, rien à faire"
        );
    }

    #[test]
    fn a_stream_cut_at_every_possible_offset_still_yields_every_object() {
        // Le vrai risque n'est pas un objet coupé une fois, c'est un flux
        // haché à un endroit qu'on n'avait pas prévu. On les essaie tous.
        let stream = br#"{"Event":"A","Data":{"x":"}{"}}{"Event":"B","Data":"{\"y\":1}"}"#;
        for cut in 0..stream.len() {
            let mut buf: Vec<u8> = Vec::new();
            let mut names = Vec::new();
            for chunk in [&stream[..cut], &stream[cut..]] {
                buf.extend_from_slice(chunk);
                loop {
                    let Some((obj, rest)) = take_frame(&buf) else {
                        break;
                    };
                    let obj = obj.to_vec();
                    let rest = rest.to_vec();
                    if let Some((n, _)) = decode(&obj) {
                        names.push(n);
                    }
                    buf = rest;
                }
            }
            assert_eq!(names, vec!["A", "B"], "coupé à l'octet {cut}");
        }
    }
}
