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
- [x] stale 페이지 주입 제외 — **구현 완료** (2026-07-29): 기준은 페이지 자신의 마지막 커밋(코드 PR 동승 머지 = 재검증; last_verified는 미커밋 페이지의 폴백·증빙용). 그 이후 커밋에서 anchors 매칭 파일이 바뀌면 stale → 요약 주입 제외 + 포인터 1줄("re-verify before relying") + systemMessage에 "N stale excluded". 미커밋 변경은 in-flight로 간주(staleness 아님). fail-open(알 수 없는 sha는 fresh)

### 구현 중 검증 항목 (PRD 미해결)

- [ ] **증류 품질 실전 판정** — 이 리포에 `coopera init` 실행(self-hosting 시작) → 실개발 세션 1회 → 생성된 위키 diff를 사용자가 직접 리뷰 = M1 완성 기준
- [x] Claude Code SessionEnd 훅의 transcript_path 실제 형식 — 확인 완료(`~/.claude/projects/<경로-슬러그>/<session-id>.jsonl`, user content는 문자열/블록 배열 혼재, assistant는 thinking/text/tool_use)
- [ ] (M2) Codex PreToolUse가 apply_patch에도 발화하는지

## 컨벤션

- 코어 로직은 coopera-core(모듈별 단위 테스트 인라인), CLI는 얇은 명령 레이어(cmd_*.rs). e2e는 crates/coopera-cli/tests/.
- fail-open 전역 원칙: 훅은 어떤 실패에도 세션을 차단하지 않는다(exit 0 + systemMessage 경고).
- 도구가 생성하는 모든 텍스트는 영어. 프로젝트 문서는 한국어.
- 계획·문서에 기간/주차 금지 — 순서(단계)만.
- 커밋: conventional commits, 메시지에 검증 상태 표기.

<!-- coopera:begin -->
## Team context (coopera)
This repository uses coopera, a harness that shares team context between AI coding sessions.
- Shared team knowledge lives in `wiki/` (concepts, modules, decisions, playbooks). Read `wiki/INDEX.md` first; do not bulk-read the whole wiki directory.
- Before planning or making design decisions, consult the injected team context and relevant wiki pages. Avoid conflicting with or duplicating in-flight teammate work; align with recorded team decisions.
- Session digests are written to `wiki/sessions/` automatically; wiki changes ride along with your code PR for human review.
<!-- coopera:end -->
