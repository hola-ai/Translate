use crate::config::Config;
use serde_json::{json, Value};
use std::time::Duration;

const TO_KOREAN: &str = "You translate World of Warcraft in-game chat from English into natural, casual Korean, the way a Korean player would write it. \
Keep player names, numbers and item names in brackets as they are. In chat, 1 usually means yes/ready. \
Reply with the translation only: no quotes, notes or romanization.";

const TO_ENGLISH: &str = "Translate the user's World of Warcraft chat message from Korean into casual English, the way an English-speaking player would type it in game chat. \
Keep player names, numbers and item names as they are. Reply with the translation only.";

const TO_CHINESE: &str = "Translate the user's World of Warcraft chat message from Korean into casual Simplified Chinese, the way a Chinese player would type it in game chat. \
Keep player names, numbers and item names as they are. Reply with the translation only.";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Direction {
    ToKorean,
    ToEnglish,
    ToChinese,
}

/// English WoW abbreviations and slang as `en=ko` pairs. Only the terms found
/// in a line are sent with it, so a long list costs no extra tokens.
const SLANG: &str = "\
lfm=파티원 구함;lfg=파티 구함;lf=구함;lf1m=1명 구함;lf2m=2명 구함;lf3m=3명 구함;lf4m=4명 구함;pug=막공;pst=귓속말 주세요;\
mt=메인 탱커;ot=서브 탱커;tank=탱커;healer=힐러;heals=힐러;heal=힐러;dps=딜러;mdps=근접 딜러;rdps=원거리 딜러;\
bis=최종 아이템;ilvl=아이템 레벨;gs=장비 점수;ms=주 특성;os=보조 특성;gdkp=골드 경매 공대;sr=소프트 예약;hr=하드 예약;ml=전리품 담당자;\
wts=판매;wtb=구매;wtt=교환;cod=대금 청구 우편;ah=경매장;gb=길드 은행;gbank=길드 은행;mats=재료;boe=착용 시 귀속;bop=획득 시 귀속;\
hc=영웅 난이도;nm=일반 난이도;lfr=공격대 찾기;m+=쐐기;bg=전장;wsg=전쟁노래 협곡;ab=아라시 분지;av=알터랙 계곡;\
mc=화산 심장부;bwl=검은날개 둥지;aq=안퀴라즈;naxx=낙스라마스;kara=카라잔;bt=검은 사원;swp=태양샘 고원;ulduar=울두아르;toc=십자군의 시험장;icc=얼음왕관 성채;\
pull=풀링;wipe=전멸;trash=잡몹;adds=추가 몹;aggro=어그로;taunt=도발;dispel=해제;aoe=광역기;cc=군중 제어;kite=몰이;oom=마나 없음;\
cd=쿨타임;cds=쿨타임;proc=발동;dot=도트;buff=버프;debuff=디버프;nerf=너프;lust=블러드러스트;bl=블러드러스트;brez=전투 부활;cr=전투 부활;rez=부활;res=부활;ress=부활;\
inv=초대;summ=소환;afk=자리 비움;brb=곧 돌아옴;bio=화장실;omw=가는 중;rdy=준비 완료;gz=축하해요;grats=축하해요;ty=고마워요;thx=고마워요;np=별말씀을요;sry=미안해요;\
gl=행운을 빌어요;wb=어서 와요;pls=부탁해요;plz=부탁해요;idk=모르겠어요;kos=보이면 공격;ninja=템 먹튀;\
pally=성기사;lock=흑마법사;hunt=사냥꾼;dk=죽음의 기사;dh=악마사냥꾼;sham=주술사;drood=드루이드;boomy=조화 드루이드;ret=징벌 성기사;prot=방어 전문화;resto=회복 전문화;disc=수양 사제;spriest=암흑 사제";

/// True when `term` appears in `text` as a whole word, so mt does not match
/// empty and lf does not match lfm. Both must be lowercase.
fn has_word(text: &str, term: &str) -> bool {
    text.match_indices(term).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        let after = text[start + term.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

/// The slang terms in a line, longest first, ignoring case. At most 12 to keep
/// prompts short.
fn slang_in(text: &str) -> Vec<(&'static str, &'static str)> {
    let text = text.to_lowercase();
    let mut found: Vec<(&str, &str)> = SLANG
        .split(';')
        .filter_map(|pair| pair.split_once('='))
        .filter(|(en, _)| has_word(&text, en))
        .collect();
    found.sort_by_key(|(en, _)| std::cmp::Reverse(en.chars().count()));
    found.truncate(12);
    found
}

/// Tencent's HY-MT models want their own prompt and no system message.
fn is_hy_mt(model: &str) -> bool {
    let m = model.to_lowercase();
    m.contains("hy-mt") || m.contains("hunyuan-mt")
}

/// Builds the request body for either prompt style.
fn request_body(model: &str, text: &str, direction: Direction) -> Value {
    if !is_hy_mt(model) {
        let system = match direction {
            Direction::ToKorean => {
                let slang: Vec<String> = slang_in(text).iter().map(|(en, ko)| format!("{en} = {ko}")).collect();
                if slang.is_empty() {
                    TO_KOREAN.to_string()
                } else {
                    format!("{TO_KOREAN} Slang in this line: {}.", slang.join(", "))
                }
            }
            Direction::ToEnglish => TO_ENGLISH.to_string(),
            Direction::ToChinese => TO_CHINESE.to_string(),
        };
        return json!({
            "model": model,
            "temperature": 0.2,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": text },
            ],
        });
    }

    // Templates and sampling settings from the HY-MT1.5 model card. Its only
    // terminology template is written in Chinese, whatever the languages.
    let terms: Vec<String> = match direction {
        Direction::ToKorean => slang_in(text)
            .iter()
            .map(|(en, ko)| format!("{en} 翻译成 {ko}"))
            .collect(),
        Direction::ToEnglish | Direction::ToChinese => Vec::new(),
    };
    let prompt = if terms.is_empty() {
        match direction {
            Direction::ToKorean => format!("Translate the following segment into Korean, without additional explanation.\n\n{text}"),
            Direction::ToEnglish => format!("Translate the following segment into English, without additional explanation.\n\n{text}"),
            Direction::ToChinese => format!("将以下文本翻译为中文，注意只需要输出翻译后的结果，不要额外解释：\n\n{text}"),
        }
    } else {
        format!(
            "参考下面的翻译：\n{}\n\n将以下文本翻译为韩语，注意只需要输出翻译后的结果，不要额外解释：\n{text}",
            terms.join("\n")
        )
    };
    json!({
        "model": model,
        "temperature": 0.7,
        "top_p": 0.6,
        // Read by llama.cpp and Ollama; other servers ignore them.
        "top_k": 20,
        "repeat_penalty": 1.05,
        "messages": [{ "role": "user", "content": prompt }],
    })
}

/// Local servers (LM Studio, Ollama) serve the API under /v1; a bare
/// `http://host:port` gets it added so a missing /v1 is not a silent failure.
fn completions_url(base: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    let after_scheme = base.split_once("://").map_or(base, |(_, rest)| rest);
    if after_scheme.contains('/') {
        format!("{base}/chat/completions")
    } else {
        format!("{base}/v1/chat/completions")
    }
}

/// Sends one message to an OpenAI-compatible /chat/completions endpoint.
pub fn translate(config: &Config, text: &str, direction: Direction) -> Result<String, String> {
    let url = completions_url(&config.api_base);
    let body = request_body(&config.model, text, direction);

    let mut request = ureq::post(&url).timeout(Duration::from_secs(60));
    if !config.api_key.is_empty() {
        request = request.set("Authorization", &format!("Bearer {}", config.api_key));
    }
    let response: Value = match request.send_json(body) {
        Ok(r) => r.into_json().map_err(|e| format!("bad JSON from {url}: {e}"))?,
        Err(ureq::Error::Status(code, r)) => {
            let detail = r.into_string().unwrap_or_default();
            return Err(format!("{url} returned {code}: {}", detail.chars().take(200).collect::<String>()));
        }
        Err(e) => return Err(format!("could not reach {url}: {e}")),
    };

    response["choices"][0]["message"]["content"]
        .as_str()
        .map(clean_reply)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            let detail = response.to_string();
            format!("{url} gave no translation: {}", detail.chars().take(200).collect::<String>())
        })
}

/// Models sometimes wrap the answer in quotes or add a thinking block; drop both.
fn clean_reply(raw: &str) -> String {
    let text = match raw.rfind("</think>") {
        Some(end) => &raw[end + "</think>".len()..],
        None => raw,
    };
    let text = text.trim();
    let text = text
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(text);
    // WoW chat is single-line.
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn abbreviations_match_whole_words_in_any_case() {
        let found = slang_in("LFM need MT and bis geared heals, Pst");
        assert!(found.contains(&("lfm", "파티원 구함")));
        assert!(found.contains(&("mt", "메인 탱커")));
        assert!(found.contains(&("bis", "최종 아이템")));
        assert!(found.contains(&("heals", "힐러")));
        assert!(found.contains(&("pst", "귓속말 주세요")));
        assert!(!found.iter().any(|(en, _)| *en == "lf"));
        assert_eq!(slang_in("the bag is empty"), vec![]);
        assert_eq!(slang_in("LFM"), vec![("lfm", "파티원 구함")]);
        assert_eq!(slang_in("M+ tonight"), vec![("m+", "쐐기")]);
    }

    #[test]
    fn cleans_reply() {
        assert_eq!(clean_reply("\"Need a healer\"\n"), "Need a healer");
        assert_eq!(clean_reply("<think>hmm</think>\nLF tank"), "LF tank");
    }

    #[test]
    fn calls_openai_compatible_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = vec![0u8; 8192];
            let n = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let body = r#"{"choices":[{"message":{"role":"assistant","content":"던전 힐러 구함"}}]}"#;
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(reply.as_bytes()).unwrap();
            request
        });
        let config = Config {
            api_base: format!("http://127.0.0.1:{port}/v1/"),
            api_key: "secret".into(),
            ..Config::default()
        };
        let korean = translate(&config, "LF healer for the dungeon", Direction::ToKorean).unwrap();
        assert_eq!(korean, "던전 힐러 구함");
        let request = server.join().unwrap();
        assert!(request.starts_with("POST /v1/chat/completions"));
        assert!(request.contains("Bearer secret"));
    }

    #[test]
    fn adds_v1_to_bare_host() {
        assert_eq!(completions_url("http://localhost:1234"), "http://localhost:1234/v1/chat/completions");
        assert_eq!(completions_url("http://localhost:1234/"), "http://localhost:1234/v1/chat/completions");
        assert_eq!(completions_url("http://localhost:1234/v1/"), "http://localhost:1234/v1/chat/completions");
        assert_eq!(completions_url("https://api.openai.com/v1"), "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn hy_mt_uses_model_card_prompt_and_glossary() {
        let body = request_body("mlx-community/HY-MT1.5-7B-8bit", "LFM need MT", Direction::ToKorean);
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1, "HY-MT takes no system prompt");
        let prompt = messages[0]["content"].as_str().unwrap();
        assert!(prompt.starts_with("参考下面的翻译：\n"));
        assert!(prompt.contains("lfm 翻译成 파티원 구함"));
        assert!(prompt.contains("mt 翻译成 메인 탱커"));
        assert!(!prompt.contains("탱커\nheal"));
        assert!(prompt.contains("翻译为韩语"));
        assert!(prompt.ends_with("不要额外解释：\nLFM need MT"));
        assert_eq!(body["top_k"], 20);

        let plain = request_body("HY-MT1.5-7B", "hello there", Direction::ToKorean);
        assert_eq!(
            plain["messages"][0]["content"],
            "Translate the following segment into Korean, without additional explanation.\n\nhello there"
        );
        let reply = request_body("HY-MT1.5-7B", "탱커 구해요", Direction::ToEnglish);
        assert_eq!(
            reply["messages"][0]["content"],
            "Translate the following segment into English, without additional explanation.\n\n탱커 구해요"
        );
        let chinese = request_body("HY-MT1.5-7B", "멈춰", Direction::ToChinese);
        assert_eq!(chinese["messages"][0]["content"], "将以下文本翻译为中文，注意只需要输出翻译后的结果，不要额外解释：\n\n멈춰");
        let general = request_body("qwen2.5:7b", "need MT", Direction::ToKorean);
        assert_eq!(general["messages"][0]["role"], "system");
        let system = general["messages"][0]["content"].as_str().unwrap();
        assert!(system.ends_with("Slang in this line: mt = 메인 탱커."), "{system}");
    }

    #[test]
    fn slang_table_is_well_formed() {
        assert!(SLANG.split(';').all(|pair| pair.split_once('=').is_some_and(|(en, ko)| {
            !en.is_empty() && !ko.is_empty() && en == en.to_lowercase()
        })));
    }
}
