# Translate — WoW 영어 채팅 실시간 한국어 번역

원본: [cherryroots/Translate](https://github.com/cherryroots/Translate) (중국어 → 영어, MIT). 이 저장소는 그 프로젝트를 **영어 → 한국어**로 바꾸고 WoW 약어 사전(LFM, PUG, PST, BiS, MT 등 100여 개)을 넣은 fork입니다. 구조와 픽셀 스트립 방식은 원본 그대로입니다.

다른 플레이어의 영어 채팅을 내 번역 모델(Ollama 또는 oMLX)로 실시간 번역해 게임 위 별도 창에 한국어로 보여 주고, 내가 친 한국어는 영어나 중국어로 바꿔 클립보드에 넣어 줍니다. 두 부분으로 되어 있습니다.

- **`addon/Translate`**: 게임 안 애드온. 리테일과 WoW: Forever 한 폴더로 동작합니다 (`## Interface: 16001, 120105`).
- **`companion`**: 게임 옆에서 돌리는 작은 Rust 앱. OpenAI 호환 API로 번역하고, 결과를 게임 위 오버레이에 그립니다.

## 동작 방식

WoW 애드온은 인터넷에 접근할 수 없고 채팅 로그 파일도 실시간으로 쓰이지 않으므로, 애드온은 화면과 클립보드를 통해 companion 앱에 줄을 넘깁니다. 키 입력을 대신 누르는 일은 없습니다.

1. **자동 번역:** 다른 플레이어의 영어 줄이 오면 애드온이 화면 왼쪽 위에 작은 색 블록 띠(256×32px, 픽셀 스트립)로 그립니다. companion 앱이 그 띠를 화면에서 찾아 읽고, 번역해서 채팅창 옆에 두는 클릭 통과형 오버레이에 한국어로 표시합니다.
2. **답장:** 게임에서 `/tr 멈춰`를 치면 같은 띠를 통해 앱으로 가고, 앱이 영어로 번역해 클립보드에 넣습니다. 오버레이에 `복사됨: Stop`이 뜨면 채팅창에 붙여넣고 보냅니다. `/tr zh 멈춰`는 중국어, `/ㅅㄱ`과 `/번역`은 한글 IME가 켜진 채로 쓰는 같은 명령입니다.
3. **게임 안 번역(선택):** `/tr links`를 켜면 영어 줄 뒤에 `[T]` 링크가 붙습니다. 클릭 → 복사 → 오버레이가 준비됐다고 하면 아래 칸에 붙여넣기 → 채팅창에 `[KO]`로 출력됩니다.

띠를 읽는 것은 화면 캡처입니다. 픽셀만 읽고 게임에 입력을 보내지 않지만, Blizzard 약관은 이를 명확히 허용하지도 금지하지도 않습니다. `/tr strip`으로 띠를 끄면 자동 번역 대신 `[T]` 링크 경로만 쓸 수 있습니다.

## 제한 사항

- **macOS에서만 확인했습니다.** macOS 26, Apple Silicon. Windows와 Linux는 원본과 같은 방법으로 빌드되지만 이 fork에서는 테스트하지 않았습니다.
- **같은 Mac에서 Ollama로 모델을 돌리려면 메모리 48GB 이상을 권장합니다.** 게임과 7B 모델을 함께 돌리기 때문입니다. 메모리가 적으면 1.8B 모델을 쓰거나, 다른 PC의 oMLX에 번역을 맡기세요.
- **WoW는 창 모드나 전체 창 모드여야 합니다.** 전용 전체 화면에서는 오버레이가 보이지 않고 화면 캡처가 검게 나옵니다.
- **리테일 12.1.5, Forever 1.60.1에서 확인했습니다.** 패치로 버전이 바뀌면 애드온이 구버전으로 표시됩니다. `/dump select(4, GetBuildInfo())` 값을 `Translate.toc`의 `## Interface:`에 추가하세요.
- **인스턴스 안 채팅은 번역되지 않을 수 있습니다.** 최신 클라이언트는 인스턴스 안 채팅을 보호된 값으로 넘겨 애드온이 읽지 못합니다.
- **모니터 색 프로필:** 캡처된 색이 모니터 프로필에 따라 바뀌는 것을 띠의 기준색으로 보정합니다. Dell AW3821DW에서 확인했고, 다른 모니터에서 띠를 못 찾으면 아래 문제 해결을 보세요.
- **번역 품질:** 약어 사전은 모델에 주는 힌트라 항상 사전대로 번역되지는 않습니다. 거래 채널처럼 줄이 빠르게 올라오면 번역이 밀립니다 (`/tr trade`로 끌 수 있습니다).
- 띠 한 장은 376바이트라 WoW 채팅 한 줄(최대 255바이트)과 채널·이름이 모두 들어갑니다. 더 길면 글자 경계에서 잘립니다.

## 빠른 시작 (간단 버전)

두 경우 모두 애드온 설치와 앱 실행은 같고, 번역 서버만 다릅니다. 앱은 [Releases](https://github.com/hola-ai/Translate/releases)에서 받거나 아래 "앱 빌드"대로 직접 빌드합니다.

### A. Ollama — 같은 Mac에서 모델을 돌릴 때

1. [Ollama](https://ollama.com/download)를 설치하고(`brew install --cask ollama`도 됩니다) 모델을 받습니다. 한 번만 하면 되고 이후 오프라인으로 동작합니다.
   ```sh
   ollama pull hf.co/tencent/HY-MT1.5-7B-GGUF:Q4_K_M     # 4.6GB, 권장
   ollama pull hf.co/tencent/HY-MT1.5-1.8B-GGUF:Q4_K_M   # 메모리가 적을 때
   ```
2. 애드온을 복사합니다 (Forever는 `_forever_`).
   ```sh
   cp -R addon/Translate "/Applications/World of Warcraft/_retail_/Interface/AddOns/"
   ```
3. 앱을 실행합니다. 기본 설정이 Ollama(`http://localhost:11434/v1`, 모델 `hf.co/tencent/HY-MT1.5-7B-GGUF:Q4_K_M`)라서 Settings를 바꿀 필요가 없습니다. 1.8B를 받았다면 Model만 그 이름으로 바꿉니다. **Test model**을 눌러 한국어 문장이 나오면 서버는 끝입니다. 화면 기록 권한을 물으면 허용합니다.
4. 게임에서 `/tr test`. 앱에 `Reading the pixel strip on screen 1 at 0,48`이 뜨고 오버레이에 한국어가 나오면 됩니다.

### B. oMLX — 다른 Mac에서 모델을 돌릴 때

1. 서버 Mac의 [oMLX](https://github.com/jundot/omlx) 관리 화면(`http://<서버>:8000/admin`)에서 `mlx-community/HY-MT1.5-7B-8bit`를 받고 API 키를 만듭니다. 두 Mac은 같은 네트워크(또는 Tailscale)에 있어야 합니다.
2. 애드온을 복사합니다 (A의 2번과 같습니다).
3. 앱을 실행하고 **Settings**에 API base URL `http://<서버>:8000/v1`, API key, Model `HY-MT1.5-7B-8bit`를 넣고 **Save settings** → **Test model**.
4. 게임에서 `/tr test`.

## 자세한 사용법

### 애드온 설치

`addon/Translate` 폴더를 플레이하는 클라이언트의 AddOns 폴더에 넣습니다. 파일이 `Translate` 폴더 안에 들어가야 합니다.

```
리테일:  World of Warcraft/_retail_/Interface/AddOns/Translate/
Forever: World of Warcraft/_forever_/Interface/AddOns/Translate/
```

애드온 목록에 안 보이면 클라이언트를 재시작합니다. 설정은 `WTF/Account/<계정>/SavedVariables/Translate.lua`에 저장되고, 켜고 끄기 값과 띠 위치만 들어 있습니다.

### 앱 빌드

Rust를 설치한 뒤 `companion/`에서 빌드합니다. macOS에서는 `cargo build --release` 한 줄이면 되고, 결과는 `target/release/translate-companion`입니다. Linux는 `.github/workflows/build.yml`에 적힌 PipeWire, X11, Wayland 개발 패키지가 필요합니다.

앱은 설정을 실행 파일 옆 `config.json`에 씁니다. API 키가 평문으로 들어가므로 이 파일은 공유하지 않습니다 (`.gitignore`에 있습니다). 처음 실행하면 macOS가 화면 기록 권한을 묻습니다. 빌드를 받아 쓰는 경우 `chmod +x`와 `xattr -d com.apple.quarantine`이 필요할 수 있습니다.

### 번역 서버

앱은 OpenAI 호환 `/chat/completions` 엔드포인트라면 무엇이든 씁니다. `http://host:port`처럼 경로 없이 적으면 `/v1`을 붙입니다.

| 서버 | API base URL | 모델 | 비고 |
| --- | --- | --- | --- |
| Ollama (기본값) | `http://localhost:11434/v1` | `hf.co/tencent/HY-MT1.5-7B-GGUF:Q4_K_M` | API 키 불필요. 1.8B: `hf.co/tencent/HY-MT1.5-1.8B-GGUF:Q4_K_M` |
| oMLX | `http://<서버>:8000/v1` | `HY-MT1.5-7B-8bit` | 네트워크 접근에는 API 키 필요 |
| LM Studio | `http://localhost:1234/v1` | LM Studio에 표시되는 이름 | 같은 GGUF를 올리고 서버 시작 |
| OpenAI 등 | `https://api.openai.com/v1` | `gpt-4o-mini` 등 | 유료, 채팅이 외부로 나감 |

모델 이름에 `hy-mt`가 들어 있으면 앱이 HY-MT1.5 모델 카드의 프롬프트와 샘플링 설정을 쓰고, 줄에서 찾은 약어를 용어 힌트로 함께 보냅니다. 다른 모델에는 약어를 설명하는 시스템 프롬프트를 씁니다. HY-MT1.5는 한국어와 중국어를 포함한 33개 언어를 지원하는 번역 전용 모델이라 짧은 채팅에 엉뚱한 설명을 덧붙이지 않아 권장합니다.

### 게임에서 쓰기

- **번역되는 채팅:** 일반, 외침, 귓속말, 파티, 공격대, 인스턴스, 길드, 관리자, 채널(거래 포함).
- **번역되지 않는 채팅:** 내가 쓴 줄, 한글이 섞인 줄, `1`처럼 영문자가 없는 줄, 보호된 채팅.
- **오버레이 배치:** 앱에서 **Unlock overlay to move it** → 원하는 곳으로 끌어 놓기 → Settings에서 크기·글자·배경 불투명도 → **Save settings** → **Lock overlay**. 잠그면 클릭이 게임으로 통과하고, 다른 창이 WoW 앞에 오면 숨습니다(**Hide when WoW is in the background**로 끌 수 있음). 왼쪽 위 띠는 가리지 않습니다. 원문도 보려면 **Show English too**.
- **답장:** `/tr <한국어>` → 영어가 클립보드에 → 오버레이에 `복사됨: ...`이 뜨면 채팅창에 붙여넣고 전송. 앱 창의 Reply 칸에 입력해도 됩니다(영어만).

| 명령어 | 하는 일 |
| --- | --- |
| `/tr <한국어>` | 영어 번역을 클립보드에 넣음. `/tr zh <한국어>`는 중국어. `/ㅅㄱ`, `/번역`도 같은 명령 |
| `/tr trade` | 거래 채널 번역 켜기/끄기 (게임 채널에는 영향 없음) |
| `/tr strip` | 픽셀 스트립 켜기/끄기 (끄면 자동 번역과 답장 모두 중지) |
| `/tr strippos <x> <y>` | 띠 위치 이동, 화면 픽셀 기준. Mac은 메뉴 막대 때문에 y 48 이상 (기본값) |
| `/tr links` | 영어 줄 뒤 `[T]` 링크 표시 (기본 꺼짐) |
| `/tr test` | 테스트 문장을 띠로 보냄. 공개 채팅에는 아무것도 안 올림 |
| `/tr probe` | 클라이언트 기능 지원 여부 출력 |
| `/tr` | 최근 영어 줄의 복사·붙여넣기 창 열기 (`<` `>`로 이동) |

### 약어 사전 고치기

`companion/src/translate.rs`의 `SLANG` 상수에 `영어=한국어;` 형식으로 들어 있습니다. 영어는 소문자로 적고 항목은 세미콜론으로 나눕니다. 단어 단위로 대소문자 구분 없이 찾으므로 `mt`가 `empty` 안에서 잡히지는 않습니다. 고친 뒤 `cargo test`로 확인하고 `cargo build --release` 후 앱을 다시 시작합니다.

### 문제 해결

| 증상 | 원인 | 조치 |
| --- | --- | --- |
| 앱에 `Looking for the pixel strip`이 계속 떠 있음 | 띠가 메뉴 막대나 다른 창에 가려짐, 전용 전체 화면, `/tr strip`이 꺼짐 | 창 모드로 바꾸고 `/tr strippos 0 48`. 스크린샷을 찍어 `translate-companion --decode 스크린샷.png`로 읽히는지 확인 |
| `Model test failed: ... 401 API key required` | API 키가 비어 있음 | Settings에 키 입력 후 저장 |
| `401 Invalid API key` | 키가 틀리거나 공백 포함 | 다시 복사 |
| 모델을 찾을 수 없다는 오류 | Model 이름이 서버와 다름 | `ollama list` 또는 `http://<서버>:8000/v1/models`의 이름으로 수정 |
| `could not reach http://localhost:11434` | Ollama가 꺼져 있음 | Ollama 앱 실행 또는 `ollama serve` |
| 오버레이가 사라짐 | 다른 창이 WoW 앞에 있음 | WoW 창 클릭, 또는 **Hide when WoW is in the background** 해제 |
| 번역이 점점 늦게 따라옴 | 거래 채널처럼 줄이 많음 | `/tr trade`로 끄기 |
| 애드온이 구버전으로 표시 | 패치로 Interface 번호가 바뀜 | `Translate.toc`의 `## Interface:`에 현재 번호 추가 |
| 오버레이 한글·한자가 네모로 나옴 | 글꼴을 못 찾음 | Mac은 AppleSDGothicNeo·PingFang, Windows는 malgun·msyh를 찾음. 없으면 `companion/src/main.rs`의 글꼴 목록에 경로 추가 |

### 개발

- 테스트: `cd companion && cargo test` (프롬프트, 약어 매칭, 띠 해독, 색 보정, 서버 호출).
- 띠 디버그: 스크린샷을 `translate-companion --decode <파일>`로 읽으면 위치와 내용이 나옵니다.
- 원본 대비 바뀐 곳: `translate.rs`(프롬프트·사전·방향), `text.rs`/`clipboard.rs`(한글 감지), `strip.rs`(색 보정, `@reply` 표식), `main.rs`(글꼴), 애드온 `Chat.lua`(영어 줄 선별, 거래 채널 필터, 답장 전송), `Core.lua`(`/tr` 명령, 별칭), `Strip.lua`(Mac 띠 위치).
- 릴리스: Actions 탭의 **Build** 워크플로를 `v1.2.3` 같은 태그로 실행하면 원본과 같은 방식으로 버전을 적고 빌드해 Release에 올립니다.

## 아이콘

`assets/icon.svg`가 원본입니다. `companion/icon.png`는 창 아이콘, `companion/icon.ico`는 Windows exe에, `addon/Translate/icon.tga`는 애드온 목록에 쓰입니다.

## 라이선스

MIT, [LICENSE](LICENSE) 참고. 원본 저작권은 cherryroots에게 있습니다.
