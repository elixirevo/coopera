# coopera

AI 코딩 도구(Claude Code·Codex·Antigravity)에 설치하는 팀 컨텍스트 하네스 — 각자의 세션에서 자동 캡처한 맥락을 LLM 위키(git)와 push 기반 트래킹으로 통합해, **모든 팀원의 LLM이 같은 프로젝트 이해 위에서 정합하게 작업**하게 한다(인수인계가 아니라 이해의 통합). 서버 0대, git이 유일한 데이터 소스.

## 문서

- 리서치: [docs/project/00-research.md](docs/project/00-research.md) · 아키텍처: [docs/project/00-harness.md](docs/project/00-harness.md) · 스파이크: [docs/project/00-spikes.md](docs/project/00-spikes.md)
- 아이디어 브리프: [docs/project/01-idea-brief.md](docs/project/01-idea-brief.md)
- 기획서: [docs/project/02-plan.md](docs/project/02-plan.md)
- PRD: [docs/project/03-prd.md](docs/project/03-prd.md) — 기능·수용 기준의 원천
- 스캐폴드 리포트: [docs/project/04-scaffold-report.md](docs/project/04-scaffold-report.md)

## 스택

Rust stable 워크스페이스(`crates/coopera-core` lib + `crates/coopera-cli` bin "coopera"). git 연동은 시스템 git 셸아웃(스파이크 검증 완료, 라이브러리 기각), 비동기 런타임 없음(훅은 초단명 프로세스), 증류는 각 에이전트 headless 호출(자체 API 키 없음). 근거: PRD 결정 로그.

## 명령 (검증된 것만 기록)

```bash
# 빌드:
cargo build
# 테스트 (유닛 15 + 스모크 1):
cargo test
# 포맷:
cargo fmt
# 실행 예 (훅이 부르는 내부 엔진 — 사용자 명령 아님):
cargo run -p coopera-cli -- status
cargo run -p coopera-cli -- wiki lint
echo '{}' | cargo run -p coopera-cli -- hook session-start
```

## M1 체크리스트 (PRD 수용 기준)

- [x] F1 `coopera init`: wiki/·.coopera/ 스캐폴드, .claude/settings.json 훅 병합(멱등), CLAUDE.md/AGENTS.md 마커 블록, 비-git 에러 — 스모크 테스트 통과
- [x] F2 세션 시작 주입: 훅 JSON I/O, 예산 상한(기본 1500), anchor 랭킹, systemMessage 가시화, fail-open — 스모크 테스트 통과 (**워킹 슬라이스**)
- [x] F3 세션 종료 증류 — **구현 완료** (2026-07-29): 에이전트 headless 호출(설정 가능, 기본 `claude -p`, 프롬프트는 stdin)→고정 스키마 JSON 파싱→다이제스트(wiki/sessions/)→위키 초안 create/update(린트 게이트+레드액션+wiki/ 경로 탈출 차단)→스테이징(코드 PR 동승)→성공 시 retro 큐 해제. 재귀 가드 COOPERA_DISTILL. 스텁 e2e + **실 `claude -p` 검증 완료**(결정 2·학습 1·고품질 초안 생성 확인)
- [x] F3 후속: 유사 페이지 update 우선(프롬프트 지시+update 액션), 실패 시 undistilled 마커·`--retro`로 소급 처리
- [x] F4 위키 린트: 스키마 검증, 위반 시 비0 종료 — 스모크 테스트 통과
- [x] stale 페이지 주입 제외 — **구현 완료** (2026-07-29): 기준은 페이지 자신의 마지막 커밋(코드 PR 동승 머지 = 재검증; last_verified는 미커밋 페이지의 폴백·증빙용). 그 이후 커밋에서 anchors 매칭 파일이 바뀌면 stale → 요약 주입 제외 + 포인터 1줄("re-verify before relying") + systemMessage에 "N stale excluded". 미커밋 변경은 in-flight로 간주(staleness 아님). fail-open(알 수 없는 sha는 fresh) — *이후 하드닝 P2에서 "제외" 대신 "[STALE — re-verify] 마커 요약 주입"으로 대체*

### 구현 중 검증 항목 (PRD 미해결)

- [x] **증류 품질 실전 판정 — 통과 (2026-07-29, M1 완료)**: self-hosting 상태에서 실개발 세션(3.7MB 트랜스크립트) 증류 → 결정 5·학습 7·위키 초안 3장 전부 린트 통과 → 사용자가 diff 리뷰 후 승인, 페이지별 원자 커밋으로 반영. 후속 개선 후보: touched 목록의 리포 밖 절대경로 필터링
- [x] Claude Code SessionEnd 훅의 transcript_path 실제 형식 — 확인 완료(`~/.claude/projects/<경로-슬러그>/<session-id>.jsonl`, user content는 문자열/블록 배열 혼재, assistant는 thinking/text/tool_use)
- [x] (M2) Codex PreToolUse가 apply_patch에도 발화하는지 — **확인 완료** (2026-07-29, M2 데모에서 `"tool_name":"apply_patch"` 실측; 2026-04의 Bash-only 보고는 구버전)

## M2 체크리스트 (PRD 수용 기준) — 완료 2026-07-29

- [x] F5 presence 발행/조회: 세션 단위 ref(`refs/coopera/presence/<user>/<session>`), SessionStart fetch(`--prune`+브랜치 헤드, 2초 타임아웃)→`.coopera/cache/presence.md` 물질화, 세션 경계는 동기 push·프롬프트는 백그라운드 push, SessionEnd 정리(원격 삭제) — bare-origin 2클론 e2e 통과
- [x] F6 UserPromptSubmit: intent 갱신(레드액션·120자) + triggers 매칭 페이지 주입(최대 3, [unreviewed] 마커, 지연 예산 위해 staleness 생략) — e2e 통과
- [x] F7 활성 브랜치 요약: 최근 14일 원격 브랜치(기본 브랜치 제외, 최대 5)를 활동 지도에 통합 — push 축 1급 신호
- [x] F8 Codex 어댑터: init이 `.codex/config.toml` `[[hooks.*]]` 3종 생성(`COOPERA_TOOL=codex`) — **크로스툴 데모 통과**: Codex가 주입된 팀 결정(git refs presence)과 안티-서베일런스 원칙대로 계획 수립, 서버 제안 0. 전제: 프로젝트 trust + `/hooks` 훅 승인 1회
- [x] F9 Antigravity 읽기 경로: AGENTS.md hook-less 지침(presence.md·INDEX 읽기) + `--retro` 확장(트랜스크립트 저장소 스캔 — 다이제스트 없는 최근 세션 최대 3건, 16KB 문턱)
- 라이브 검증 보너스: M2 작업 중 병렬 실세션이 종료되며 **자동 증류가 무인으로 작동** — 결정 4페이지(003~006) 스테이징됨

## 신뢰성 하드닝 P0~P2 — 완료 2026-08-02

진단 배경: 2026-07-29 이후 자동 증류 성공 0건. 원인 3중주 — ① SessionEnd의 백그라운드 distiller가 훅 프로세스 그룹 정리에 사살됨(수동 실행은 102초 성공), ② 실패가 완전 무음(stderr → /dev/null, 로그·카운터 없음), ③ 안전망 `--retro`를 아무도 자동 호출하지 않음. 부가로 Codex presence 키가 훅마다 달라져 죽은 ref가 원격에 영구 잔류, stale 과잉 제외로 주입팩 빈곤화.

- [x] P0 증류 소생: spawn을 자기 프로세스 그룹으로 detach(`spawn::detach`), distiller 출력 `.coopera/cache/distill.log`(512KB 캡), session-start에 "N undistilled pending" 표시, session-start가 `distill --retro` 자동 드레인(락파일 가드·stale 30분 해제·회당 에이전트 호출 최대 3건), 타임아웃 180→600s, retro 스캔 하드닝(mtime 최신순, 활성 세션 제외 = mtime 10분/presence 30분, distiller 트랜스크립트 PROMPT_MARKER 스니핑 제외)
- [x] P1 Codex/presence: 훅 명령에 `COOPERA_SESSION_FALLBACK="ppid-$PPID"` — 세션 키가 훅 간 안정화되어 intent 갱신·SessionEnd 정리 작동(e2e 통과; **실 Codex 세션에서 $PPID 안정성 실측은 대기**), last_seen 24h 초과 ref lazy GC(로컬 즉시 + 원격은 백그라운드 push 1회, 파싱 불가 ref는 보존)
- [x] P2 주입/계측: stale 페이지는 제외 대신 [STALE — re-verify] 마커로 요약 주입(fresh 이후 순위, 예산 초과분만 포인터), `.coopera/cache/metrics.jsonl` 계측(inject/distill/distill_error, 1MB 로테이션, 로컬 전용 — M3 백로그 선행)
- [x] 도구별 distiller 선택 (결정 003 완성): 세션 호스팅 도구(COOPERA_TOOL, `--tool` 관통·큐에 기록)가 자기 에이전트로 증류 — claude-code→`claude -p`, codex→`codex exec --ephemeral -s read-only -c model_reasoning_effort="low" -o <응답파일> -`(**codex-cli 0.146.0 실측**: stdin 프롬프트·`-o` 응답·ephemeral 롤아웃 미생성 확인), antigravity→`agy --effort low -p <프롬프트>`(**agy 실측**: 프롬프트는 argv 전용·stdin 미지원, 커스텀 페르소나 설정에서도 고정 스키마 JSON 준수 확인). 해당 CLI 부재 시 설치된 다른 에이전트로 폴백(단일 도구 사용자도 증류 작동), 설정 오버라이드는 `[distill]`(전역)·`[distill.agents.<tool>]`(도구별) — 플레이스홀더: COOPERA_OUT=응답 파일, COOPERA_PROMPT=argv 프롬프트(120KB 상한 가드)
- [x] Codex 롤아웃 증류 경로 — **구현 완료** (2026-08-02): session_meta 첫 줄로 포맷 감지(`codex_meta`) → 전용 추출기 `extract_codex_rollout`(response_item만 사용 — event_msg는 스트리밍 중복, developer/reasoning/시스템 주입 패킷 스킵, apply_patch에서 touched 추출·세션 cwd로 상대경로 해석), retro 스캔이 `~/.codex/sessions/` date-키 스토어를 cwd 매칭으로 커버(가드 동일: 16KB·14d·mtime·presence·다이제스트 dedup·마커 스니핑은 128KB 헤드 — 첫 줄이 12–44KB), 다이제스트 tool="codex"·세션 id는 payload.id. 실 롤아웃(cli 0.104·0.146) 추출 검증 + 유닛 2·retro e2e 1 추가. 머지 시 결정 003과 연동: 스캔 finds가 도구 태그를 달고 나와 Codex 롤아웃은 codex 에이전트로 증류, 포맷 감지가 잘못 라벨된 큐 항목의 tool을 교정
- [x] F8b Antigravity 훅 어댑터 — **구현 완료** (2026-08-02, 읽기 전용 경로 졸업): init이 `.agents/hooks.json`에 "coopera" 네임드 훅 병합(네이티브 머지 — 팀의 다른 훅 보존). PreInvocation = 첫 모델 호출에만 팀 컨텍스트를 `injectSteps[].ephemeralMessage`로 주입 + presence(키=페이로드 conversationId, intent=트랜스크립트 tail의 USER_REQUEST), Stop = 롤링 트랜스크립트 큐 마킹(quiescence 가드: mtime 10분 — 회전 중인 대화는 증류 유예, 다이제스트 존재 시 재큐잉 안 함) + retro spawn. 추출기는 step_index/source 포맷 감지(`antigravity_detect`/`extract_antigravity` — USER_REQUEST 언랩, tool_calls 경로 인자에서 touched), 큐 3필드화(경로·도구·세션 — transcript.jsonl 파일명 문제 해결). **실측**: agy 1.1.9 임베디드 훅 가이드에서 계약 추출(camelCase 페이로드·트러스트 게이트·헤드리스 `-p`는 훅 미발화 확인), 실 트랜스크립트 포맷 검증. **남은 검증 1개: 인터랙티브 Antigravity 세션에서 훅 발화** — 트러스트된 이 리포에서 대화 1번 후 `.coopera/cache/metrics.jsonl`의 `"source":"pre-invocation"` 이벤트로 확인
- [ ] 후속: touched 목록의 리포 밖 절대경로 필터링(M1 이월). Antigravity 과거 세션 소급(`~/.gemini/<product>/brain/<id>/.system_generated/logs/transcript.jsonl` 스캔 — 워크스페이스 매칭 방법 필요)

## 컨벤션

- 코어 로직은 coopera-core(모듈별 단위 테스트 인라인), CLI는 얇은 명령 레이어(cmd_*.rs). e2e는 crates/coopera-cli/tests/.
- fail-open 전역 원칙: 훅은 어떤 실패에도 세션을 차단하지 않는다(exit 0 + systemMessage 경고).
- 도구가 생성하는 모든 텍스트는 영어. 프로젝트 문서는 한국어.
- 계획·문서에 기간/주차 금지 — 순서(단계)만.
- 커밋: conventional commits, 메시지에 검증 상태 표기.
- **커밋되는 설정에 머신 고유 경로 금지**: 훅 명령은 `${COOPERA_BIN:-coopera}`로 PATH를 경유한다(스모크 테스트가 절대 경로 유입을 회귀 검사). 로컬 빌드를 쓰려면 셸 프로필에 `COOPERA_BIN`을 export.
- self-hosting 개발 루프: 코드 수정 → `cargo build --release` → `cp target/release/coopera ~/.local/bin/`(또는 `COOPERA_BIN` 지정) → 다음 세션부터 반영.

<!-- coopera:begin -->
## Team context (coopera)
This repository uses coopera, a harness that shares team context between AI coding sessions.
- Shared team knowledge lives in `coopera/` (concepts, modules, decisions, playbooks). Read `coopera/INDEX.md` first; do not bulk-read the whole wiki directory.
- Before planning or making design decisions, consult the injected team context and relevant wiki pages. Avoid conflicting with or duplicating in-flight teammate work; align with recorded team decisions.
- Session digests are written to `coopera/sessions/` automatically; wiki changes ride along with your code PR for human review.
- If your tool does not run coopera hooks, start each task by reading `.coopera/cache/presence.md` (teammate activity map) and `coopera/INDEX.md`.
<!-- coopera:end -->
