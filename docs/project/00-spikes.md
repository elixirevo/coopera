# 스파이크 결과 — 핵심 가정 검증

- 수행일: 2026-07-28, 환경: macOS 로컬 (Claude Code 2.1.126 / Codex CLI 0.145.0 / Antigravity 설치본)
- 결론: **핵심 가정 4건 전부 통과.** 설계 변경 불필요, 스캐폴딩 진행 가능.

## ① 커스텀 ref push (presence 아키텍처 전제) — PASS

`refs/coopera/presence/<user>/<session>`에 presence.yaml 커밋을 생성해 GitHub(private repo) 대상 전체 수명주기 검증:

| 동작 | 결과 |
|---|---|
| ref 생성 후 push | ✅ new reference 수락 |
| `git ls-remote 'refs/coopera/*'` | ✅ 조회됨 |
| 타 개발자 관점 fetch (`+refs/coopera/*:refs/coopera/*`) | ✅ presence.yaml 내용 복원 |
| force-push 갱신 (presence 리프레시) | ✅ forced update 수락 |
| ref 삭제 (세션 정리) | ✅ deleted |

기본 GitHub 설정에서 push ruleset 차단 없음. (조직 리포에서 ruleset이 있는 경우는 도입 시 확인 항목으로 유지)

## ② 주입 순응도 (가장 중요한 가정) — PASS

가짜 프로젝트(src/payments/retry.js — Redis 락 사용 주석 포함)에서 `claude -p` 3회 대조 실험. 태스크: "결제 재시도에 지수 백오프 추가 계획 수립". 주입 내용: A가 pay-idem 브랜치에서 멱등 리팩터링 중 + 팀 결정 "Redis 락 → DB 유니크 제약". **모델은 의도적으로 최약체(haiku)** — 약한 모델이 순응하면 상위 모델은 더 확실.

| 런 | 조건 | 결과 |
|---|---|---|
| 1 | 무주입 (대조군) | ❌ "Redis 락 획득 후 재시도" — **팀 결정과 정면 충돌하는 계획** 생성 (우리가 막으려는 바로 그 사고) |
| 2 | 활동 지도만 주입 (지시문 없음) | ✅ "A의 DB 제약 기반 멱등성 작업과 조율" 단계 자발 생성, Redis 락 재도입 없음 |
| 3 | 활동 지도 + 지시문 1줄 | ✅ 저장 방식을 팀 결정에 정렬 + "A의 pay-idem 브랜치와 조율, 작업 중복 금지" 명시 |

시사점: **정보만으로도 행동이 바뀐다**(런2). 지시문은 조율을 명시화하는 보너스(런3) — 비용이 거의 없으므로 유지. 주입 문구 초안이 그대로 프로덕션 후보.

## ③ Codex 훅 (어댑터 전제) — PASS

- 스키마 확정 (공식 문서 + 실검증): 프로젝트 `.codex/config.toml` 인라인
  ```toml
  [[hooks.SessionStart]]
  [[hooks.SessionStart.hooks]]
  type = "command"
  command = "..."
  ```
  또는 `.codex/hooks.json`. 이벤트명은 Claude Code와 동일(PascalCase): SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, SessionEnd, Stop 등 11종.
- 실발화 확인: **SessionStart · UserPromptSubmit · PreToolUse 3종 모두 발화** (`codex exec`, matcher ".*").
- `additionalContext` 주입: SessionStart/UserPromptSubmit/PreToolUse/PostToolUse 지원 (공식 문서) — coopera 주입 채널 확보.
- 핸들러: command만 실행(prompt/agent는 파싱만) → 증류는 `codex exec` 셸아웃으로.
- **전제 조건 2개**: ① 프로젝트가 `[projects]` trust_level="trusted" 여야 프로젝트 훅 로드 ② 훅 정의 자체의 신뢰 승인(인터랙티브 `/hooks`) 필요 — 자동화는 `--dangerously-bypass-hook-trust`. 온보딩 문서에 반영할 것.
- 미확인 잔여: PreToolUse가 apply_patch(편집 툴)에도 발화하는지 — 도그푸딩에서 확인.
- 참고: 사용자 기본 reasoning effort가 max면 exec가 매우 느림 → coopera가 호출할 땐 `-c model_reasoning_effort="low"` 오버라이드.

## ④ Antigravity 통합 표면 — 확정 (훅 없음, 폴백 성립)

- VS Code 파생 데스크톱 앱. 라이프사이클 훅 개념 없음(바이너리 검증: PreToolUse 등 0건).
- **AGENTS.md + GEMINI.md 읽기 지원 확인** (workbench 바이너리 + `~/Library/Application Support/Antigravity/User/GEMINI.md` 존재).
- 어댑터 전략 확정: **읽기 = AGENTS.md 컴파일 / 쓰기 = 소급 증류(다음 훅 가능 세션에서) / 작업 트래킹 = push 축(브랜치·커밋)** — 설계의 폴백 경로가 그대로 성립.

## ⑤ 골든 세트 — 진행 중

도그푸딩 세션 트랜스크립트가 쌓이는 대로 증류 프롬프트 회귀 평가 세트 구축 (차단 아님).

## 부수 산출물

- GitHub 리포 생성·푸시 완료: github.com/elixirevo/coopera (private)
- coopera 리포가 Codex trusted 프로젝트로 등록됨 (`~/.codex/config.toml`)
- 스파이크용 `.codex/config.toml`(훅 마커 테스트)이 리포에 있음 — 스캐폴딩 시 실제 훅으로 교체 예정
