# coopera 사용 가이드

AI 코딩 도구(Claude Code · Codex · Antigravity)에 설치하는 팀 컨텍스트 하네스의 사용법.
핵심 원칙: **사용자 명령 0개** — 설치 후에는 평소처럼 코딩하면 되고, 사람이 하는 일은 위키 diff 리뷰뿐이다.

## 1. 무엇이 자동으로 일어나는가

```
세션 시작 ──→ [주입] 팀 활동 지도 + 위키 요약이 에이전트 컨텍스트에 들어감
프롬프트  ──→ [주입] 트리거 단어와 매칭된 위키 페이지 요약 추가 주입
작업 중   ──→ [presence] 누가·어느 브랜치에서·무슨 의도로 작업 중인지 git refs로 공유
세션 종료 ──→ [캡처] 트랜스크립트를 각자의 에이전트로 증류 → 다이제스트 + 위키 초안
          ──→ [스테이징] 위키 diff가 git add 되어 다음 커밋(코드 PR)에 동승
다음 시작 ──→ [자가 치유] 놓친 세션을 retro가 자동 소급 증류 (회당 최대 3건)
```

모든 데이터는 git 안에만 있다(서버 0대, 텔레메트리 없음). 증류는 각자 도구의 CLI를 headless로 호출하므로 별도 API 키가 없고 비용은 각자 구독에 귀속된다.

## 2. 설치

### 리포에 처음 도입 (팀당 1회)

```bash
# 1) 바이너리 설치 — Releases에서 받거나:
cargo build --release && cp target/release/coopera ~/.local/bin/

# 2) 리포 루트에서:
coopera init
```

`init`이 만드는 것 (멱등 — 재실행해도 안전):

| 경로 | 역할 | 커밋? |
|---|---|---|
| `coopera/` | 팀 위키 (INDEX + concepts/modules/decisions/playbooks + sessions/) | O |
| `.coopera/config.toml` | 예산·증류 설정 (기본값이면 그대로 두면 됨) | O |
| `.coopera/cache/` | 로컬 임시 (presence.md, 로그, 큐, 지표) | X (gitignored) |
| `.claude/settings.json` | Claude Code 훅 3종 병합 (기존 설정 보존) | O |
| `.codex/config.toml` | Codex 훅 3종 (관리 블록) | O |
| `.agents/hooks.json` | Antigravity 훅 2종 ("coopera" 키만 소유, 다른 훅 보존) | O |
| CLAUDE.md · AGENTS.md | coopera 지침 마커 블록 | O |

커밋해서 push하면 팀 전체에 배포된다.

### 팀원 합류 (사람당 1회)

바이너리만 PATH에 설치하면 끝. **설치하지 않은 팀원의 도구에서는 훅이 조용히 통과**되므로(read-only 모드) 아무것도 깨지지 않는다. PATH 밖의 로컬 빌드를 쓰려면 셸 프로필에:

```bash
export COOPERA_BIN="$HOME/dev/coopera/target/release/coopera"
```

## 3. 도구별 사용 프로세스

### Claude Code

전제 조건 없음 — init 후 바로 작동한다.

1. **세션을 열면** 화면에 한 줄이 보인다 (가시화 원칙 — 이 줄이 보이면 작동 중):
   ```
   coopera: injected 9 items (~917 tokens), 1 teammate signal(s), 6 stale flagged (re-verify)
   ```
   에이전트 컨텍스트에는 이런 블록이 들어가 있다:
   ```
   <team-context source="coopera">
   ## Active teammate work
   - Elixir · codex · branch main — Propose a repository-local presence mechanism (active, 5m ago)

   ## Team knowledge (coopera/)
   - [decision] [unreviewed] Distillation by each agent, not centralized — Each tool distills
     its own sessions... (coopera/decisions/003-distillation-by-each-agent-not-centralized.md)
   - [decision][STALE — re-verify] Presence via git custom refs — ... (coopera/decisions/001-...)

   ## Guidance
   Before planning or making design decisions, review the team context above. ...
   </team-context>
   ```
2. **프롬프트에 트리거 단어**(페이지 frontmatter의 `triggers:`)가 들어가면 해당 페이지 요약이 추가 주입된다:
   ```
   coopera: +2 page(s) matched this prompt
   ```
3. **세션을 정상 종료**(`/exit`, 창 닫기)하면 백그라운드에서 `claude -p`로 증류가 돈다(1~2분).
   강제 종료해도 다음 세션 시작 때 retro가 소급 처리한다.
4. **다음 커밋 때** `git status`에 스테이징된 위키 diff가 보인다 → 리뷰 후 코드와 함께 커밋.

### Codex

전제 조건 (1회): 프로젝트 **trust** + `/hooks`에서 훅 정의 **승인**.

- 주입·캡처 흐름은 Claude Code와 동일 (SessionStart/UserPromptSubmit/SessionEnd).
- 세션 키는 훅 명령이 넣어주는 `ppid-$PPID`로 안정화된다 (Codex 페이로드에 세션 id가 없음).
- 증류는 `codex exec --ephemeral -s read-only -c model_reasoning_effort="low"`로 돈다.
- Codex 세션의 롤아웃(`~/.codex/sessions/`)은 retro 스캔이 세션 cwd로 이 리포 소속을 판별해 소급 증류한다 — SessionEnd를 놓쳐도 캡처된다.

### Antigravity (IDE / `agy` 대화 모드)

전제 조건 (1회): 폴더 **트러스트** (처음 열 때 승인하면 `trustedWorkspaces`에 기록됨).

- **대화의 첫 모델 호출 직전**(PreInvocation)에 팀 컨텍스트가 transient 메시지로 주입되고, presence가 발행된다 (키 = conversationId, intent = 마지막 사용자 요청).
- **매 턴이 끝날 때**(Stop) 대화가 캡처 후보로 마킹된다. Antigravity는 "세션 종료" 개념이 없으므로, **대화가 10분 이상 조용해지면** retro가 `agy`로 증류한다.
- 주의: 헤드리스 `agy -p`는 훅을 발화하지 않는다(실측) — 인터랙티브/IDE 세션 전용이며, 이 덕분에 증류용 agy 실행이 훅 재귀를 일으키지 않는다.

## 4. 하루 사용 범례 (시나리오)

> A가 오전에 Claude Code로 payments 모듈의 락 전략을 바꾸고 종료했다.
> B가 오후에 Codex로 같은 모듈 작업을 시작한다.

1. A의 세션 종료 → 자동 증류 → `coopera/decisions/012-lock-strategy-....md` 초안이 A의 작업 브랜치에 스테이징됨.
2. A가 diff 리뷰 후 코드와 함께 커밋·push. (PR 리뷰어는 코드와 지식 변경을 한 번에 본다.)
3. B가 세션을 열면: presence에 A의 최근 활동 + 위키에 새 결정이 주입됨.
4. B가 "락 재시도 로직 고쳐줘"라고 치면 `triggers: [lock, retry]` 매칭으로 해당 결정 요약이 다시 주입됨.
5. B의 에이전트는 A의 결정과 모순되는 코드를 만들지 않는다. **이것이 전부다 — 둘 다 coopera 명령을 한 번도 치지 않았다.**

증류 결과물(다이제스트) 예시 — `coopera/sessions/`에 자동 생성:

```markdown
# Session digest — 019fab6b-cfdc-7bb1-94d5-6d4240a856ef (codex)
author: Elixir · 2026-08-02T00:05:34Z
intent: Propose a near-real-time repository-local mechanism for teammates to
see each other's active coding sessions.
decisions:
- Use git custom refs under refs/coopera/presence/* instead of a pub/sub server ...
learnings:
- ...
touched: crates/coopera-core/src/presence.rs, ...
```

## 5. 위키 리뷰 — 사람이 하는 유일한 일

- 자동 생성/수정된 페이지는 전부 `confidence: draft`로 시작하고 주입 시 `[unreviewed]` 마커가 붙는다.
- **승격**: 내용을 읽고 맞으면 frontmatter를 `confidence: high`로 바꿔 커밋한다. 이때부터 마커 없이 주입된다.
- **stale**: 페이지의 `anchors:` 경로 코드가 페이지 마지막 커밋 이후 바뀌면 `[STALE — re-verify]` 마커로 주입된다. 내용을 재확인하고 페이지를 커밋에 태우면(내용 수정이든 확인 후 그대로든) 재검증된다.
- 직접 지식을 추가하려면 해당 디렉터리에 페이지를 만들고 커밋하면 된다. 스키마는 기존 페이지를 참고하고, 게이트는:
  ```bash
  coopera wiki lint
  ```

## 6. 확인과 문제 해결

정상 작동의 신호는 **세션 시작의 한 줄 메시지**다. 그 외 점검 도구:

```bash
coopera status
```

| 파일 (`.coopera/cache/`) | 내용 |
|---|---|
| `presence.md` | 팀 활동 지도 (Antigravity 등 hook-less 도구도 읽는 파일) |
| `distill.log` | 백그라운드 증류의 전체 출력 — "증류가 안 되는 것 같을 때" 첫 확인처 |
| `undistilled.log` | 미증류 큐 (경로·도구·세션) — retro가 자동으로 비운다 |
| `metrics.jsonl` | inject/distill 이벤트 계측 (로컬 전용) |

자주 겪는 상황:

- **"N undistilled session(s) pending"이 보인다** → 정상. 다음 세션 시작마다 retro가 최대 3건씩 소급 증류한다. 원인이 궁금하면 `distill.log`.
- **주입 줄이 아예 안 보인다** → `command -v coopera`(PATH 확인), Codex는 trust+`/hooks` 승인, Antigravity는 폴더 트러스트 여부 확인.
- **훅이 뭔가 실패한다** → coopera는 전역 fail-open: 어떤 실패도 코딩 세션을 막지 않고 경고 한 줄로 강등된다. 세션이 멈췄다면 coopera 문제가 아니다.
- **증류 에이전트를 바꾸고 싶다** → `.coopera/config.toml`의 `[distill]`(전역) 또는 `[distill.agents.<tool>]`(도구별). 예: 더 싼 모델로
  ```toml
  [distill]
  command = "claude"
  args = ["-p", "--model", "claude-haiku-4-5"]
  ```

## 7. 참고: 명령어는 왜 안 쓰나

`coopera`의 서브커맨드(`init`·`status`·`wiki lint`·`distill --retro`·`hook *`)는 훅이 부르는 내부 엔진이다. 사람이 칠 일이 있는 것은 도입 시 `init`, CI/수동 검증용 `wiki lint`, 디버깅용 `status` 정도이며, 나머지는 전부 자동이다.
