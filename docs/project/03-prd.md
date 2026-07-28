---
project: "coopera"
status: confirmed        # 사용자가 순서 0~3 연속 실행을 사전 위임(2026-07-28) — 뼈대는 완료 보고에서 사후 확인
purpose: "내부 도구 우선 → 향후 전체 공개 실사용 도구"
created: 2026-07-28
pipeline_stage: 03-prd
based_on: 02-plan.md
stack: "Rust(stable) 단일 바이너리 CLI · git이 유일한 데이터 소스 · 훅 어댑터(Claude Code/Codex) + AGENTS.md 컴파일(Antigravity) · 증류는 각 에이전트 headless 호출"
---

# coopera — PRD

## 개요

AI 코딩 도구(Claude Code·Codex·Antigravity)에 설치하는 하네스. 세션에서 자동 캡처한 팀 맥락을 LLM 위키(git)와 presence refs로 통합하고, 훅이 매 세션에 주입해 **모든 팀원의 LLM이 같은 프로젝트 이해 위에서 작업**하게 한다. 비즈니스 배경은 [02-plan](02-plan.md), 아키텍처 근거는 [00-harness](00-harness.md) 참조. 사용자 명령 0개 원칙 — `coopera` CLI는 훅이 부르는 내부 엔진이다.

## 사용자와 핵심 시나리오

- **JS-A (주입)**: 팀원이 어제 payments 모듈의 락 전략을 바꿨을 때, 오늘 내가 그 모듈 작업을 시작하면, 내 에이전트가 그 결정을 이미 알고 계획을 세워서, 모순된 코드를 만들지 않게 하고 싶다.
- **JS-B (캡처)**: 내 세션에서 중요한 결정이 나왔을 때, 내가 문서를 따로 쓰지 않아도, push와 함께 팀 위키에 초안이 실려서, 리뷰만 하면 팀 지식이 되게 하고 싶다.

## 기능 요구사항 (마일스톤 연동)

### M1 — 지식 루프 1회전 ※ 상세 수용 기준

| ID | Job Story | 수용 기준 | 구현 중 검증 |
|---|---|---|---|
| F1 `coopera init` | 새 리포에 coopera를 도입할 때, 명령 한 번으로 하네스가 설치되길 원한다 | - [ ] `wiki/`(INDEX+4유형 디렉터리+샘플), `.coopera/config.toml`, `.coopera/cache/`(gitignored) 생성<br>- [ ] `.claude/settings.json`에 SessionStart/SessionEnd 훅 병합 기록(기존 설정 보존, 이미 있으면 멱등)<br>- [ ] CLAUDE.md·AGENTS.md에 coopera 섹션 컴파일(마커 블록으로 갱신 가능)<br>- [ ] git 리포가 아니면 명확한 에러 | |
| F2 세션 시작 주입 | 세션을 시작할 때, 팀 활동·위키 요약이 자동으로 컨텍스트에 들어오길 원한다 | - [ ] `coopera hook session-start`: stdin의 훅 JSON을 읽고 `additionalContext` JSON 출력<br>- [ ] 주입 팩 = L0(INDEX 요약+활동 지도)+L1(anchor 매치 페이지 summary), **총 토큰 예산 상한 준수**(기본 1,500: presence 300/위키 800/지침 200/여유 200, config로 조정)<br>- [ ] stale 페이지 주입 제외(포인터만)<br>- [ ] `systemMessage`로 1줄 가시화("coopera: injected N items")<br>- [ ] git/네트워크 실패 시 **fail-open**(빈 주입+경고 1줄, 세션 차단 금지)<br>- [ ] 로컬 처리 100ms 이내(fetch 제외) | 주입 문구는 스파이크 ② 검증본(활동 지도+지시문)을 영어로 사용 |
| F3 세션 종료 증류 | 세션이 끝났을 때, 결정·학습이 다이제스트와 위키 diff로 자동 정리되길 원한다 | - [ ] `coopera hook session-end`: transcript 경로를 받아 `coopera distill` 비동기 실행<br>- [ ] 증류는 `claude -p`(사용자 에이전트) 호출, 고정 스키마 다이제스트(≤30줄: 의도1/결정각2/학습각1/접촉 표면) 생성 → `wiki/sessions/`<br>- [ ] 실질 내용(결정·학습) 있으면 위키 diff(새 페이지 또는 기존 수정)를 **작업 브랜치에 스테이징**, 없으면 다이제스트만<br>- [ ] 유사 기존 페이지 있으면 새 페이지 대신 수정 diff 우선<br>- [ ] 시크릿 패턴 레드액션 후 기록<br>- [ ] 증류 실패 시 세션 종료를 막지 않음(fail-open, 미증류 마커 남김) | **증류 품질** — M1 완료 직후 사용자 리뷰가 첫 판정. 골든 세트는 M3 |
| F4 위키 린트 | 위키가 커질 때, 페이지가 규율을 지키길 원한다 | - [ ] `coopera wiki lint`: summary·anchors 필수, 본문 ≤100줄, frontmatter 스키마(type/anchors/triggers/summary/last_verified/confidence/source) 검증<br>- [ ] 위반 시 비0 종료+파일:줄 리포트 | |

### M2 — push 축 + 크로스툴 ※ 한 줄 정의

| ID | Job Story (한 줄) |
|---|---|
| F5 | presence 발행/조회: 세션 단위 ref(`refs/coopera/presence/<user>/<session>`) push, SessionStart fetch→`.coopera/cache/presence.md` 물질화 |
| F6 | UserPromptSubmit: 프롬프트에서 intent 추출→presence 갱신(로컬 즉시·push는 다음 기회), triggers 매칭 개념어 summary 주입 |
| F7 | 활성 브랜치 요약: push된 브랜치의 커밋·diffstat를 활동 지도에 통합(push 축 1급 신호) |
| F8 | Codex 어댑터: `.codex/config.toml` 훅 생성(init에 통합), 동일 주입/증류(`codex exec -c model_reasoning_effort="low"`) |
| F9 | Antigravity 읽기 경로: AGENTS.md 컴파일 강화 + 소급 증류(`coopera distill --retro`: 미증류 세션 감지·처리) |

### M3 이후 — 백로그

PreToolUse advisory(캐시 대조 경고) · staleness CI(anchors 변경 감지→stale 배지) · 주입 계측(`metrics.jsonl`) · 골든 세트 회귀 평가 · MCP 래퍼 · wiki bootstrap(콜드 스타트) · 플러그인 마켓플레이스 패키징(M4). 명시적 제외 유지: 허브 서버, 충돌 레이더, 소프트 클레임, 멀티 리포, 임베딩 검색.

## 비기능 요구사항 (최소셋)

- **성능**: 훅 로컬 처리 100ms 이내(UserPromptSubmit 필수), SessionStart는 fetch 포함 3초 이내(git 타임아웃 1.5초, 초과 시 캐시 사용+fail-open).
- **안전**: 어떤 실패도 코딩 세션을 차단하지 않는다(fail-open 전역 원칙). 발행 전 시크릿 레드액션(API 키·토큰·비밀번호 패턴, `.env` 값). presence는 intent 요약 1~2줄만.
- **데이터**: 모든 영속 데이터는 리포 안(git). `.coopera/cache/`는 로컬 임시(재생성 가능). 원격 전송은 git push 외 없음(텔레메트리 없음).
- **언어**: 도구가 생성하는 모든 텍스트(주입·다이제스트·위키 초안·CLI 메시지)는 영어.

## 기술 요구사항 ※ 스캐폴딩 게이트

### 스택 (선택 + 이유)

| 영역 | 선택 | 선택 이유 | 기각한 대안과 트레이드오프 |
|---|---|---|---|
| 언어·런타임 | **Rust stable, 단일 바이너리** | 훅마다 프로세스 기동 → 콜드 스타트가 성능 예산의 전부. 런타임 의존성 0 (2026-07-28 사용자 확정) | Go(동급이나 사용자가 Rust 지정), Node/Python(런타임 기동 수십~수백 ms로 예산 초과) |
| CLI 프레임워크 | clap v4 (derive) | 사실상 표준, 서브커맨드 구조화 | 수제 파싱(유지보수 비용) |
| git 연동 | **시스템 `git` 셸아웃** (`std::process::Command`) | 커스텀 refs·fetch·hash-object 전부 plumbing 명령으로 검증 완료(스파이크 ①). 구현 단순·거동 예측 가능 | gix/git2 라이브러리(컴파일 무게·API 학습 비용, M1 이득 없음. 성능 병목 확인되면 M3+에서 재검토) |
| 직렬화 | serde + serde_yaml(frontmatter·presence) + toml(설정) + serde_json(훅 I/O) | 각 포맷의 표준 조합 | 단일 포맷 강제(각 표면의 기존 관례를 따르는 게 마찰 적음) |
| 시간 | jiff | 시간대 처리 안전, 최신 표준 | chrono(가능하나 jiff가 후속 표준) |
| 에러 | anyhow(bin) + thiserror(core) | 관례 | — |
| 비동기 | **없음(전부 동기)** | 훅 프로세스는 초단명. 증류 비동기는 OS 백그라운드 프로세스 spawn으로 충분 | tokio(불필요한 무게) |
| 증류 LLM | 각 에이전트 headless: `claude -p`, `codex exec -c model_reasoning_effort="low"` | 자체 API 키 없음(확정). 비용 각자 귀속 | 자체 키(도입 마찰·비용 부담) |

### 아키텍처 개요 (M1 수직 슬라이스)

```
cargo workspace
├─ crates/coopera-core   # lib: wiki 모델·frontmatter, 주입 팩 빌더(랭킹·예산), git 래퍼, 레드액션, 다이제스트 스키마
└─ crates/coopera-cli    # bin "coopera": init | hook <event> | distill | wiki lint | activity | status
```

데이터 흐름(M1): `SessionStart 훅` → `coopera hook session-start`(stdin JSON) → core가 wiki/INDEX+anchor 매치 로드 → 예산 적용 → `{additionalContext, systemMessage}` JSON stdout. `SessionEnd 훅` → `coopera hook session-end` → 백그라운드 `coopera distill --transcript <path>` spawn → `claude -p`로 다이제스트 생성 → sessions/ 기록 + 위키 diff 스테이징.

훅 등록 형태(검증 완료 표면): Claude Code `.claude/settings.json` hooks / Codex `.codex/config.toml` `[[hooks.EventName]]`(M2) / Antigravity 없음→AGENTS.md 컴파일. 설정: `.coopera/config.toml`(예산·언어·레드액션 패턴 오버라이드).

### 개발 환경과 배포 대상

- 개발·배포: macOS 로컬(사용자 환경, cargo 1.93 확인), `cargo build --release` + `coopera init`이 훅에 바이너리 절대 경로 기록. 배포 패키징(homebrew/플러그인 동봉)은 M4.
- 예산: 0원(서버·외부 서비스 없음). CI는 GitHub Actions 무료 티어(fmt·clippy·test·wiki lint).
- 도그푸딩: coopera 리포 자체(self-hosting) — M1 완료 시점부터 이 리포에서 coopera가 켜진 상태로 개발.

## 지표와 이벤트

02 성공 지표의 이벤트 번역 (기록처: `.coopera/cache/metrics.jsonl`, 로컬 전용):

| 이벤트 | 발생 시점 | 판정 방법 |
|---|---|---|
| `inject` {items, tokens, latency_ms} | SessionStart/UserPromptSubmit 주입 시 | 1차 검증: 3도구 각각에서 inject 기록+가시화 확인 |
| `distill` {decisions, learnings, wiki_diff} | 증류 완료 시 | 1차 검증: 실세션에서 wiki_diff≥1 생성, 사용자 리뷰 통과 |
| `cross_tool_reflect` (수동 판정) | 데모 시나리오 수행 시 | 도구 A의 결정이 도구 B 계획에 반영됨을 세션 로그로 확인 |
| `coherence_check` (수동 판정) | M3 병렬 세션 실험 | 중복 구현·모순 결정 0건 |

## 결정 로그

| 단계 | 결정 | 이유 |
|---|---|---|
| 01·02 승계 | 본질=이해의 통합 / 2속도(위키+presence) / push 축 / 설치형 하네스 / Rust / 증류=각 에이전트 / 서버 0대 / M1=지식 루프 1회전 | [01](01-idea-brief.md)·[02](02-plan.md) 결정 로그 참조 |
| 03 | git 연동은 셸아웃(라이브러리 기각) | plumbing 명령으로 스파이크 검증 완료, M1 단순성 우선. 병목 시 M3+ 재검토 |
| 03 | 비동기 런타임 없음, 증류는 프로세스 spawn | 훅은 초단명 프로세스 — tokio 무게 불필요 |
| 03 | 워크스페이스 2크레이트(core lib + cli bin) | 테스트 용이성(core 단위 테스트)과 단순성의 균형 |
| 03 | 예산 배분 기본값: 1,500 = presence 300/위키 800/지침 200/여유 200 | 00-harness §2.4 승계, config로 조정 가능 |
| 03 | 갭 질문 0개 | 스택·플랫폼·취향 결정이 전 단계에서 모두 확정됨 |

## 미해결 질문 (구현 중 검증)

- 증류 품질(F3) — M1 직후 사용자 리뷰가 첫 판정, M3 골든 세트로 회귀화
- Codex PreToolUse의 apply_patch 발화 여부(F8) — M2 실측, 미발화 시 advisory 없이 주입·증류만
- Claude Code SessionEnd 훅이 전달하는 transcript 경로 형식 — F3 구현 중 실측(문서상 존재 확인됨)

## 다음 단계

- 04 스캐폴딩: **F1(`coopera init`)부터** — 워크스페이스 생성 → core 모델 → F2 주입(워킹 슬라이스: 수용 기준 첫 항목의 end-to-end) → F3 증류 스텁 → 스모크 테스트.
- 이 문서 기재 사항은 재질문 불필요. 단, 새 의존성 추가·스키마 대변경 등 제품 레벨 신규 결정은 사용자 확인 필요.
- 사용자의 첫 행동(M1 완료 후): coopera 리포에서 실개발 세션 1회 → 생성된 위키 diff 직접 리뷰(증류 품질 첫 판정).
