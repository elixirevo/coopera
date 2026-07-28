# 하네스 설계 v2 — LLM 위키 + check-before-start presence

- 작성일: 2026-07-27 (v1 → v2 같은 날 개정)
- 입력: [00-research.md](00-research.md) + 하네스 프레임워크 20종·MCP 실시간 능력 리서치(2026-07-27 웹 검증)
- v2 개정 사유 — 설계 결정 2건 반영:
  1. **공유 개념어·맥락 레이어를 "리포 내 LLM 위키"로 통합** (v1의 glossary/decisions/memory/sessions 4분할 디렉터리를 위키 페이지 유형으로 흡수)
  2. **실시간성 요구 완화**: MCP 구독으로 데이터를 읽는 게 아니라, **각 개발자가 작업 시작 전에 다른 개발자들의 작업을 참고할 수 있으면 충분** → 결합 지점이 5개에서 2개로 줄고, 서버 없는 MVP가 가능해짐

---

## 1. 결론 요약 (v2)

- **본질: 인수인계가 아니라 이해의 통합.** coopera는 팀원의 작업을 이어받게 하는 도구가 아니다. 각자 자기 작업을 하되, **모든 팀원의 LLM이 같은 프로젝트 이해(개념어·구조·결정·서로의 진행)를 염두에 두고** 작업하게 만들어, 산출물이 전체적으로 하나의 맥락 안에서 정합하게 나오게 하는 도구다. 위키 = 그 공유 이해의 정본(단일 멘탈 모델), presence/push 트래킹 = 상대 LLM의 작업에 대한 상호 인지, 주입 = 매 세션 그 이해 위에서 시작하게 하는 강제 장치.
- **v2의 구조**: `느린 차선 = LLM 위키(wiki/, git·PR로 승격되는 영속 지식)` + `빠른 차선 = presence 보드(지금 누가 무엇을, 세션 단위 갱신)` + `하네스 = 훅 어댑터(결합 지점 2개: 세션 시작·세션 종료)` + `인터페이스 = coopera CLI 1급, 파일이 기본 읽기 경로, MCP는 얇은 래퍼(§3.4)`.
- **컨텍스트 오염은 규율로 막는다**: 주입은 "목차+요약까지, 본문은 조회"의 점진 공개 + 하드 토큰 예산(§2.4). 기록은 고정 스키마·페이지 린트·중복 병합으로, 낡음은 last_verified로, 모순은 사람이 해소.
- **제품 형태는 '설치형 하네스'다 — 별도 CLI 도구가 아니다.** Claude Code에는 플러그인(훅·스킬·statusline·MCP 번들), Codex/Cursor에는 훅 설정 번들로 리포에 심어지고, 개발자는 평소처럼 `claude`/`codex`를 실행할 뿐이다. `coopera` CLI는 훅이 내부적으로 호출하는 엔진이지 사용자 인터페이스가 아니다.
- **공유·트래킹의 축은 push다** — 시간대가 아니라. push 순간 내 작업 맥락(브랜치·커밋 + 세션 의도 + 위키 diff)이 팀에 공개되고, 팀원의 하네스는 세션 시작/fetch 시점에 이를 흡수해 같은 맥락으로 작업하게 한다.
- **완화 결정의 의미**: "작업 시작 전 참고"가 요구의 전부라면, 실시간 레이어에서 라이브 충돌 레이더·TTL 클레임·asyncRewake 같은 v1의 무거운 부분이 전부 **옵션**으로 강등된다. 남는 필수품은 "세션 시작 시 팀 활동 지도를 강제 주입 + 내 의도를 발행"뿐이다.
- **기술 현실과의 일치**: 이 완화는 우연히도 조사 결과와 정확히 맞는다 — Claude Code·Cursor·Codex 모두 MCP 구독/알림을 모델 턴에 반영하지 못하므로(§8 근거), 구독 기반 설계는 애초에 불가능했다. "시작 전 pull"은 유일하게 실제로 작동하는 방식이기도 하다.
- **백엔드 추천**: presence 보드는 **git 커스텀 refs(섀도 refs)로 시작(서버 0대)** → 실시간 요구가 실제로 올라가면 같은 스키마를 경량 MCP 허브로 승격. 에이전트가 보는 인터페이스(coopera CLI, 선택적으로 MCP 래퍼)는 처음부터 고정해서 백엔드 교체가 무통이 되게 한다.

---

## 2. 느린 차선 — LLM 위키 (`wiki/`)

> 목적: 팀의 공통 개념어와 맥락을 **에이전트와 사람이 같은 표면에서** 보게 한다. 에이전트에게는 주입/조회 대상, 사람에게는 브라우징 가능한 위키.

### 2.1 구조

```
wiki/
  INDEX.md                  # 자동 생성 목차 + 최근 변경 + stale 배지
  concepts/<term>.md        # 개념어 사전: 정의, 관련 심볼, 반례·금기 (유비쿼터스 언어)
  modules/<path-slug>.md    # 모듈 가이드: 책임, 불변조건, gotcha, 주요 흐름
  decisions/NNN-<slug>.md   # 결정 기록(경량 ADR): 배경, 선택, 기각 대안, 영향 범위
  playbooks/<task>.md       # 반복 작업 절차: "마이그레이션 추가하는 법" 등
  sessions/                 # 세션 다이제스트(원천 피드, append-only) — 위키 페이지의 출처
```

위키 = 정제된 "현재 상태"(canonical), sessions/ = 출처가 되는 원천 피드. 페이지는 항상 출처 세션을 링크한다.

### 2.2 페이지 메타데이터 (frontmatter)

```yaml
---
title: 결제 멱등성
type: concept            # concept | module | decision | playbook
anchors: [src/payments/**, PaymentService]   # 코드 앵커 — 관련성 랭킹·신선도 검사의 기준
triggers: [멱등성, idempotency, 재시도, retry] # 프롬프트에 등장하면 주입 (OpenHands 방식)
last_verified: 3f2a91c   # 이 커밋 기준으로 검증됨 — 신선도의 근거
confidence: high         # high | draft(자동 초안, 미리뷰)
source: sessions/2026-07-27-a-payments.md
---
```

### 2.3 유지 파이프라인 (사람에게 문서화를 시키지 않는다)

1. **캡처**: `SessionEnd` 훅이 세션을 증류 → `sessions/` 다이제스트 생성(의도·결정·학습·접촉 표면).
2. **초안**: 증류기가 다이제스트에서 위키 후보를 추출 — 새 결정 페이지, 기존 모듈 가이드 수정 diff, 새 개념어. `confidence: draft`로 표시.
3. **승격**: 기본은 **코드 PR 동승** — 증류된 위키 변경을 작업 브랜치에 스테이징해 개발자의 코드 PR에 함께 싣는다(리뷰어가 코드와 결정을 한 번에 봄, 별도 PR 홍수 방지). 코드 PR이 없는 세션(탐색 등)만 `wiki` 라벨 별도 PR로 폴백. 어느 쪽이든 **사람이 리뷰·머지해야 정식 팀 지식**(Cursor의 auto-Memories 철회 교훈: 리뷰 없는 학습 컨텍스트는 신뢰받지 못한다).
4. **신선도**: CI(또는 훅)가 각 페이지 anchors의 파일 변경을 감지 → `last_verified`가 뒤처지면 INDEX에 stale 배지 + 재검증 태스크 생성. 낡은 지식이 조용히 주입되는 것을 막는 핵심 장치.

증류의 실행 주체는 **각 개발자의 에이전트 자신**이다 — coopera는 자체 LLM 키를 갖지 않는다(2026-07-28 확정). 도구별 어댑터: Claude Code는 훅의 agent/prompt 핸들러 또는 `claude -p` headless, Codex는 command 핸들러가 `codex exec` 호출(Codex 훅은 command 핸들러만 실행), 어댑터 없는 도구(Antigravity 등)는 **다음 세션 시작 시 소급 증류**로 폴백(크래시 복구와 같은 메커니즘 — 보편 폴백). 팀원별 모델 차이로 증류 품질이 분산되므로 고정 스키마·린트가 평준화 장치가 되고, 골든 세트 평가는 도구별로 수행한다.

### 2.4 컨텍스트 오염 방지 — 주입·조회·기록의 규율

오염은 4가지 형태로 온다: **양적 과부하**(주입이 작업 컨텍스트 예산을 잠식) / **관련성 오염**(무관한 팀 맥락이 모델 주의를 흐림 — "context rot") / **낡음 오염**(틀린 지식 주입은 없느니만 못함) / **모순 오염**(페이지 간·페이지-코드 간 불일치). 각각 다른 장치로 막는다.

**읽기 — 3단계 점진 공개(progressive disclosure) + 하드 예산:**

| 단계 | 언제 | 무엇을 | 크기 |
|---|---|---|---|
| L0 | 항상 (`SessionStart`) | INDEX 한 줄 요약 + presence 활동 지도 | ~수백 토큰 |
| L1 | 조건부 (anchor/trigger 매치) | 매치 페이지의 **summary(1~3줄)만** | 페이지당 ~50토큰 |
| L2 | 요청 시 | 본문 — 에이전트가 직접 pull | 필요한 만큼 |

- 주입 팩 **하드 토큰 예산**: 기본 ~1,500토큰(설정 가능) — presence ≤300 / 위키 요약 ≤800 / 지침 ≤200. 초과 시 랭킹 하위부터 탈락.
- **랭킹은 단순 스코어링으로 충분**(임베딩 불필요): anchor 경로 매치 > trigger 정확 매치 > 최근성 > confidence. `stale` 페이지는 주입 제외(포인터로만 존재 알림).
- 선례: Claude Code 스킬의 점진 공개(이름+설명만 상시 로드), Agent OS `index.yml`(매칭 표준만 로드), claude-mem(압축 인덱스만 주입), ACE-FCA(의도적 압축 — 리서치를 ~200줄로).
- **전체 스캔 금지 규율**: 파일 직접 읽기 경로가 열려 있으므로(§3.4) CLAUDE.md 지침에 "위키는 INDEX→필요 페이지만, 디렉터리 통째 읽기 금지"를 명시. 페이지가 작아야(아래 린트) 이 규율이 지켜진다.

**쓰기 — 위키가 쓰레기장이 되지 않게:**

- 세션 다이제스트는 **고정 스키마 + 길이 상한(≤30줄)**: 의도 1줄 / 결정 각 2줄 / 학습 각 1줄 / 접촉 표면 목록. "요약의 요약" 원칙.
- **페이지 린트**(초안 PR의 CI 게이트): summary 필수, anchors 필수, 본문 ≤100줄, 한 페이지 한 주제.
- **중복·모순 처리**: 증류기가 초안 생성 전 기존 페이지와 유사도 검사 → 새 페이지보다 **기존 페이지 수정 diff를 우선** 제안. 모순 감지 시 양쪽 주장을 PR 본문에 병기해 사람이 해소(자동 덮어쓰기 금지).
- `sessions/` 다이제스트는 원천 피드일 뿐 **주입 대상이 아니다** — 주입은 정제된 위키만.

**계측 — 오염을 측정 가능하게:** 주입 팩 크기 로깅 + 에이전트가 실제로 pull한 페이지 추적. 일정 기간 안 읽힌 페이지는 랭킹 하향·아카이브 후보(사용량 기반 큐레이션).

### 2.5 선례 대비 차별점

| 선례 | 무엇을 증명 | coopera 위키와 차이 |
|---|---|---|
| **CodeAlmanac** (YC S26, 2026-07 출시) | "트랜스크립트→리포 내 위키"가 시장 수요임을 갓 증명 | 1인 로컬 스캔·macOS 한정. coopera는 팀 파이프라인(PR 승격)+트리거/anchor 메타데이터+presence 결합+크로스툴 |
| **DeepWiki** (Cognition) | 자동 리포 위키의 유용성 | 코드에서만 생성(대화 맥락 없음), 클라우드 읽기 전용. coopera는 세션에서 나온 "왜"를 담고 리포에 커밋 |
| **Cline Memory Bank** | 리포 내 마크다운 메모리 관행 | 고정 4~5파일, 자동화·리뷰·신선도 없음 |
| **ACE-FCA thoughts** | 팀 공유 git 지식 리포 + locator 서브에이전트 | research/plan 문서 중심, 개념어·신선도·트리거 주입 없음 |

조사 결과 **개념어(유비쿼터스 언어) 레이어를 출하한 하네스는 0개** — 위키의 concepts/가 이 무주공산을 차지한다.

---

## 3. 빠른 차선 — presence 보드 ("작업 시작 전 참고")

> 요구: 구독해서 실시간으로 읽는 게 아니라, **작업을 시작할 때 다른 개발자들이 뭘 하고 있는지 참고**할 수 있으면 된다.

### 3.1 요구 완화가 바꾸는 것

| | v1 (라이브 조정) | **v2 (check-before-start)** |
|---|---|---|
| 결합 지점 | 5개 (시작/프롬프트/편집 전/편집 후/종료) | **2개 (세션 시작, 세션 종료)** + 옵션 2개 |
| 필수 기능 | presence, TTL 클레임, 충돌 레이더, 이벤트 피드 | **presence 항목 발행·조회뿐** |
| 백엔드 | 상시 허브 서버 | **git만으로 가능 (서버 0대)** |
| 신선도 | 초 단위 | 세션 단위(분~시간) — 요구상 충분 |

### 3.2 presence 항목 스키마

```yaml
user: a@team          session: s-7f3a   tool: claude-code
branch: pay-idempotency   worktree: ../wt-pay
started: 2026-07-27T09:12+09:00   last_seen: 11:40
intent: "결제 재시도 멱등 처리 리팩터링 — Redis 락 제거 방향"
anchors: [src/payments/, PaymentService]
task: BD-142          status: active   # active | wrapping-up | done
```

- `intent`는 처음엔 "세션 시작"으로 등록되고, 첫 프롬프트가 들어오면 `UserPromptSubmit` 훅이 요약해 갱신(레드액션 후 1~2줄만).
- 신선도는 `last_seen` 기반 표시로 해결: "3시간 전부터 활성", "어제 항목(stale)". 하드 만료보다 정직하다.
- **항목은 user가 아니라 세션 단위** — 병렬 워크트리로 에이전트를 4~8개 돌리는 것이 이미 업계 관행이므로 한 사람이 동시에 여러 항목을 가진다. 활동 지도는 user로 그룹핑해 표시.
- **활동 지도의 재료는 presence ref만이 아니다** — 공유의 기본 축은 push이므로, **푸시된 활성 브랜치**(커밋 메시지·diffstat 요약)가 1급 신호다. presence ref는 "아직 push 전인 의도"를 보완하는 역할.

### 3.3 백엔드 옵션

**옵션 A — git 커스텀 refs (추천 시작점, 서버 0대):**
- 세션마다 전용 ref `refs/coopera/presence/<user>/<session>`에 presence 파일 하나를 커밋해 force-push. **세션별 ref 분리라 머지 충돌이 원천 불가능**(공유 브랜치 tip 없음). 종료·만료된 세션 ref는 훅이 정리.
- 읽기: `SessionStart` 훅이 `git fetch origin 'refs/coopera/presence/*'` 후 로컬에서 조합(1~2초).
- 선례: Entire가 세션 기록을 섀도 브랜치(`entire/checkpoints/v1`)로, Git AI가 git notes refs로 — "git을 사이드채널 전송로로 쓰기"는 검증된 패턴.
- 한계: push 권한 필요, 일부 조직의 push ruleset이 커스텀 ref를 막을 수 있음(→ 일반 브랜치 `coopera-presence` 폴백 또는 옵션 B). 리포 단위 스코프.

**옵션 B — 경량 MCP 허브 (승격 경로):**
- v1의 허브 축소판: presence CRUD + 조회만. 단일 바이너리(SQLite+Streamable HTTP).
- 승격 트리거: ① 세션 중간 갱신·충돌 경고가 실제로 필요해질 때 ② 멀티 리포 팀 ③ push 권한 문제. 스키마 동일 → 이식 무통.

### 3.4 인터페이스 계층: CLI-first — 파일이 기본 읽기 경로, MCP는 얇은 래퍼

v2 초안은 MCP를 "조회 API"로 뒀지만 재검토 결과 **이 구조에서 MCP는 필수가 아니다.** 데이터 소스는 git 하나뿐이고, 두 레이어 모두 파일로 물질화되기 때문이다:

| 데이터 | 진실원천 | 에이전트 읽기 경로 | 쓰기 경로 |
|---|---|---|---|
| 위키 | `wiki/` (git) | **파일 직접** (Read/Grep) + 훅 주입 | SessionEnd 증류기 → 초안 PR |
| presence | `refs/coopera/presence/*` | 훅이 **캐시 파일**(`.coopera/cache/presence.md`, gitignored)로 물질화 → 주입 + Read | 훅 → `coopera` CLI push |
| (허브 승격 후) 라이브 신호 | 허브 | CLI/MCP 조회 | CLI/MCP |

구성 원칙:

- **`coopera` CLI가 1급 인터페이스 — 단, 개발자용 명령이 아니라 내부 엔진이다.** 사용자 관점의 제품은 Claude Code/Codex에 설치하는 하네스(플러그인·훅 번들)이고, CLI는 훅과 에이전트(Bash)가 호출하는 실행부다: `coopera announce` / `coopera activity` / `coopera wiki search` / `coopera distill`. 대상 도구(Claude Code·Cursor·Codex) 전부 셸 실행이 가능하므로 커버리지 손실이 없다. 선례: **Beads가 정확히 이 구조**(bd CLI 1급 + beads-mcp 래퍼).
- **MCP 서버는 CLI의 얇은 래퍼(선택 설치).** 존재 이유는 세 가지로 한정된다: ① Bash가 없거나 제한된 클라이언트 대응 ② **연산 조회의 편의** — `wiki search`는 frontmatter(anchors·신선도·confidence)를 이해하고 stale을 걸러 top-k만 반환한다. 맨 grep이 못 하는 부분(단 MVP에서는 잘 만든 INDEX.md로 대부분 대체 가능) ③ **허브 승격 시의 계약** — 백엔드가 git→허브로 바뀌어도 에이전트-facing 표면이 유지되는 보험.
- 즉 "MCP가 읽어오는 것은 무엇인가"의 답: **위키와 다른 별도 데이터가 아니다.** 같은 git 데이터에 대한 *연산*(랭킹·신선도 필터)과 *액션*(발행·PR 생성)이고, 그것도 CLI와 동일 기능의 다른 문일 뿐이다. 데이터 소스는 끝까지 하나(git)로 유지한다.
- **이중 보장**(MCP Agent Mail의 실패 교훈 — "에이전트가 메일함 확인을 잊는다"): ① 훅이 강제로 조회·주입(에이전트가 안 물어봐도 보게 됨) ② CLAUDE.md 지침("계획 수립 전 `coopera activity` 확인")으로 자발 호출도 유도.
- "작업 시작"은 세션 시작만이 아니다 — `UserPromptSubmit`이 새 의도 전환을 감지하면 presence를 갱신하고 로컬 캐시로 활동 지도를 재주입한다(네트워크 불필요).

### 3.5 옵션 (요구가 올라가면 켜는 것)

- `PreToolUse`(Edit|Write): 세션 시작 시 받아둔 **로컬 스냅샷**과 대조해 "A가 이 디렉터리 작업 중(09:12 시작)" advisory 경고 — 네트워크 0, 비용 거의 0이라 MVP에 넣어도 무방.
- 클레임(soft lock), 충돌 레이더, 주기적 재fetch, statusline presence 표시, asyncRewake: 전부 B 승격 이후의 옵션.

### 3.6 흐름 시나리오 (push 축)

1. A 세션 시작 → 훅이 presence 발행("payments 멱등 리팩터링") + A의 에이전트에 팀 활동 주입.
2. A가 작업 후 **push** → 브랜치·커밋과 세션 의도, 증류된 위키 diff("Redis 락 대신 DB 유니크 제약" 결정)가 함께 팀에 공개된다. **push가 곧 공유의 순간.**
3. B 세션 시작 → 훅이 fetch → "A: pay-idempotency 브랜치에서 src/payments/ 진행 중 + 결정 1건"이 주입됨 → B의 계획이 payments를 포함하면 **계획 단계에서** 에이전트가 겹침을 지적하고, A의 결정과 같은 맥락으로 작업한다.
4. A의 PR 머지 → 위키 페이지 승격 → 이후 누구든 payments를 건드리면 `SessionStart`에서 그 결정이 anchor 매치로 주입 — **모든 에이전트가 같은 결정을 알고 부팅한다.**

---

## 4. 하네스 결합 지점 v2 (훅 와이어링)

| 시점 | Claude Code | 필수/옵션 | 하는 일 |
|---|---|---|---|
| 세션 시작 | `SessionStart` | **필수** | presence fetch·주입 + 내 presence 발행 + 위키 INDEX/anchor 매치 주입 |
| 첫/새 프롬프트 | `UserPromptSubmit` | **필수** | intent 추출→presence 갱신 + triggers 매칭 개념어 주입 |
| 편집 직전 | `PreToolUse` | 옵션(저비용) | 로컬 스냅샷 대조 advisory 경고 |
| 세션 종료 | `SessionEnd` | **필수** | presence done + 세션 증류 → 다이제스트 + 위키 초안 PR |

- Cursor 대응: `sessionStart` / `beforeSubmitPrompt` / `preToolUse` / `sessionEnd` (전부 Stable). Codex: Claude Code와 동일 어휘의 hooks(`SessionStart`/`UserPromptSubmit`/`Stop`) — 어댑터 얇음.
- 배포: 훅 설정 + `.mcp.json`을 리포에 커밋 → git clone만으로 팀 전체 설치. Claude Code는 플러그인 마켓플레이스로 한 번 더 포장 가능.

---

## 5. MVP 컷 v2 (서버 0대로 시작, 구현 순서만 — 기간 없음)

- **단계 1 — 위키**: `wiki/` 스캐폴드 + frontmatter 스키마 + 페이지 린트 + CLAUDE.md/AGENTS.md 컴파일 + `SessionEnd` 증류기(고정 스키마 다이제스트 → 위키 초안, 코드 PR 동승).
- **단계 2 — presence + CLI**: `coopera` CLI(announce/activity/wiki search/distill) + 세션 단위 refs 발행/조회 + 캐시 물질화 + `SessionStart`/`UserPromptSubmit` 훅(주입 예산 포함).
- **단계 3 — 마감**: `PreToolUse` advisory, staleness CI, 주입 팩 계측, Codex·Antigravity·Cursor 어댑터, MCP 래퍼(선택), 데모 시나리오(§3.6).
- **엔진 언어: Rust** (2026-07-28 확정 — 훅 성능 예산을 충족하는 단일 바이너리).
- **검증 환경**: 1인 로컬에서 Claude Code·Codex·Antigravity 3도구로 데모 시나리오 실작동 확인 — 1차 성공 기준(상세: 01-idea-brief '성공 기준').

## 6. 리스크·미결 질문

1. **커스텀 ref 푸시 정책** — GitHub/GitLab push ruleset이 `refs/coopera/*`를 허용하는지 실환경 검증 필요(막히면 일반 브랜치 폴백/허브 승격).
2. **intent 프라이버시** — 프롬프트 요약이 presence에 실림: 레드액션 + 1~2줄 제한 + 옵트아웃 필요.
3. **위키 비대화** — 페이지 수 증가 시 INDEX 랭킹·아카이브 정책. anchors 없는 페이지 금지 같은 린트 규칙 검토.
4. **세션 중간 표류** — A와 B가 시작 후 같은 영역으로 흘러가는 경우는 다음 참고 시점까지 못 잡음(요구상 수용). 이 불만이 실제로 커지는 순간이 허브(B) 승격 시점.
5. Codex `PreToolUse`가 Bash 툴에만 발화한다는 보고(2026-04) — 현행 빌드 재확인.
6. **전체 스캔 행동** — 파일 직접 읽기 경로가 열려 있어 에이전트가 위키를 통째로 읽어버릴 수 있음: 페이지 길이 린트 + "INDEX→필요 페이지만" 지침 + pull 계측으로 완화(§2.4).

## 7. v1에서 유지되는 조사 결과 (요약)

- 유사 하네스 20종 중 2속도 결합은 없음. 최근접: Beads(2속도 데이터 플레인, 태스크 한정)·Entire(git 섀도 브랜치 세션 기록, 주입 없음)·ACE-FCA thoughts(팀 git 레이어, 라이브 없음)·Aviator(멀티플레이어 세션, git 산출물 없음)·Claude Code Agent Teams(프리미티브 완비, 1인·1머신 하드 스코프).
- 아무도 안 하는 3가지: 개념어 레이어(0/20), 크로스 개발자 다이제스트 루프("내 에이전트가 동료 에이전트의 어제 결정을 알고 부팅"), 크로스 유저 2속도 결합. — v2는 셋 다 정면으로 겨냥.

## 8. "구독 불가"의 근거 (v1 검증 유지)

- Claude Code: `resources/subscribe` 미지원(#7252), 서버 알림 수신하나 미표시(#33679), sampling 미지원(#1785). Channels는 리서치 프리뷰 + 바쁘면 다음 턴 큐잉.
- Cursor·Codex: 구독/알림 반영 미문서화·없음. 4개 도구 모두 모델은 툴 호출 시와 턴 경계에서만 MCP 데이터를 본다.
- 차기 MCP 스펙(2026-07-28 확정): 스테이트리스 우선, 세션 제거, sampling 폐기, 장수명 알림은 `subscriptions/listen`으로 — "서버가 물고 푸시"는 표준 방향과도 어긋남.

## 9. 사용자 흐름 (end-to-end)

| 단계 | 개발자에게 보이는 것 | 뒤에서 일어나는 일 |
|---|---|---|
| 온보딩(1회) | 리드: `coopera init` 후 커밋. 팀원: clone 후 첫 세션에서 훅·MCP 승인 1회 | wiki/ 스캐폴드, 훅 설정, CLAUDE.md/AGENTS.md 컴파일. (선택) `wiki bootstrap`으로 모듈 가이드 시딩 |
| 세션 시작 | 시작 메시지 1줄: "팀 컨텍스트 주입 — 활성 2명 · 새 결정 1 · 관련 개념 1" + statusline 표시 | fetch refs → 캐시 물질화 → 랭킹·예산 적용 → L0/L1 주입 → 내 presence 발행 |
| 작업 중 | 에이전트가 팀 정의·결정을 반영해 계획하고, 겹침이 있으면 이유를 말하며 조정 제안 | `UserPromptSubmit`: intent 갱신 + 트리거 매칭 주입. `PreToolUse`: 캐시 대조 advisory |
| 세션 종료 | 없음(백그라운드) — 다음 커밋 때 위키 변경이 이미 스테이징돼 있음 | async 증류 → 다이제스트 + 위키 diff를 작업 브랜치에 스테이징(PR 동승) + presence 정리 |
| 리뷰·머지 | 코드 PR 안에 결정·가이드 diff가 함께 보임. 머지 = 팀 지식 승격 | 머지 시 last_verified 갱신, INDEX 재생성 |
| 팀원의 다음 세션 | "어제 A가 payments에서 DB 유니크 제약 결정" 요약이 주입된 채 시작 | anchor 매치 랭킹이 새 결정을 상위 배치 |
| 신규 입사자 | clone 직후 위키 브라우징 + 첫 세션부터 동일 주입 — 온보딩 가속 | — |

핵심 UX 원칙: **개발자의 명시적 행동은 0개** (설치 1회와 PR 리뷰 제외). 모든 캡처·주입·발행은 훅의 부산물이고, 대신 무엇이 주입됐는지는 항상 1줄로 보여준다(invisible한 하네스는 신뢰도 가치 체감도 못 얻는다).

## 10. 개선 백로그 (v2.2)

이번 흐름 검토에서 **설계에 즉시 반영**한 것:
1. **presence를 세션 단위 ref로**(§3.2·3.3) — 병렬 워크트리 4~8개가 업계 관행인데 user당 1항목은 설계 결함이었음.
2. **위키 승격은 코드 PR 동승이 기본**(§2.3) — 하루 수 건씩 별도 초안 PR이 쌓이는 리뷰 피로를 구조적으로 차단하고, 코드와 결정을 한 번에 리뷰.
3. **주입 가시화**(§9) — 세션 시작 systemMessage 1줄 + statusline + `coopera status`.

후보 (우선순위순):
4. **콜드 스타트**: 도입 첫 1~2주는 위키가 비어 가치가 안 보임 → `wiki bootstrap`(코드베이스에서 모듈 가이드 자동 시딩, DeepWiki 방식) + "1일차 가치는 presence, 2주차 가치는 위키"로 기대 설계.
5. **초안 피로 관리**: 실질성 문턱(결정·학습이 없으면 다이제스트만 남기고 위키 diff 생성 안 함), 같은 페이지를 건드리는 오픈 초안끼리 병합, 2단 신뢰 등급(decision은 리뷰 필수 / gotcha는 draft 뱃지 달고 주입하되 주기 배치 승인).
6. **증류 신뢰성·비용**: 증류는 async 백그라운드 + 소형 모델. 크래시/강제종료 세션은 다음 `SessionStart`가 미증류 세션을 감지해 소급 증류.
7. **stale 재검증의 부산물화**: stale 페이지의 anchors를 만진 세션의 증류기가 재검증 diff를 자동 제안 — 재검증을 별도 잡무가 아니라 관련 작업의 부산물로 만든다(방치 방지).
8. **안티-서베일런스 원칙 명문화**: presence는 조율용이지 성과 측정용이 아님 — 시간 통계 제공 안 함, intent 요약만, opt-out 보장. 감시로 느껴지는 순간 데이터 자체가 사라진다(Amp 리더보드 논란의 교훈).
9. **트리거 양언어 관행**: "멱등/idempotency"처럼 영·한 쌍 triggers를 린트로 권고(키워드 매칭의 동의어 미스 완화).
10. **가치 지표**: `coopera stats` 주간 리포트 — 주입 횟수, 위키 pull 횟수, 겹침 경고·회피 사례. 도입 효과를 측정 가능하게.

## 11. 참고 링크

- Claude Code: code.claude.com/docs/en/hooks · /agent-teams — 이슈 #7252, #33679, #1785
- Cursor hooks: cursor.com/docs/agent/hooks · Codex: developers.openai.com/codex/config-reference
- MCP: modelcontextprotocol.io/specification/2025-11-25 · blog.modelcontextprotocol.io(2026-07-28 RC)
- 선례: entireio/cli(섀도 브랜치) · git-ai-project/git-ai(notes refs) · AlmanacCode/codealmanac(트랜스크립트→위키) · docs.openhands.dev/overview/skills(트리거 주입) · Dicklesworthstone/mcp_agent_mail(자발 호출 의존의 실패) · gastownhall/beads · humanlayer ACE-FCA · docs.claude-mem.ai(캡처→재주입 루프)
