# AI 코딩 협업 컨텍스트 공유 — 문제 분석과 기존 프로젝트 조사

- 작성일: 2026-07-27
- 방법: 4축 병렬 웹 리서치 (① 팀 공유 메모리 ② 세션 공유·프로버넌스 ③ 리포 네이티브 조정 ④ 조직 컨텍스트 엔진·시장 공백). 모든 프로젝트 현황·날짜·스타 수는 2026-07-27 기준 웹 검증 결과.
- 목적: coopera 프로젝트의 문제 정의, 해결 방향, 경쟁 지형 파악. 다음 단계(`01-idea-brief.md`)의 입력.

---

## 1. 문제 구조화

> "여러 개발자가 각자 AI 코딩 도구를 쓰며 협업할 때, 서로의 개발 맥락을 몰라 코드를 효율적으로 개발하지 못한다."

### 1.1 "개발 맥락"의 5가지 실체

| 종류 | 내용 | 예시 |
|---|---|---|
| **의도 (Intent)** | 지금 무엇을, 왜 만들고 있는가 | "결제 재시도 로직을 멱등하게 바꾸는 중" |
| **결정 (Decision)** | 무엇을 택했고 무엇을 기각했는가, 제약조건 | "Redis 락 대신 DB 유니크 제약 — 운영 단순성 때문" |
| **학습 (Learning)** | 코드베이스에 대해 알아낸 사실 | "이 테스트는 타임존 때문에 플레이키함" |
| **규약 (Convention)** | 팀의 패턴·스타일·금기 | "API 에러는 반드시 Result 타입으로 감쌀 것" |
| **계획 (Plan)** | 다음 작업과 작업 간 의존성 | "auth 리팩터링이 끝나야 세션 캐시 작업 가능" |

### 1.2 맥락이 사는 세 장소 — 그리고 새로 생긴 세 번째 장소

1. **리포지토리** (코드·테스트·문서) — git으로 공유되지만, 사후적이고 결과만 남는다.
2. **개발자의 머리** — PR·회의·문서로 공유. 원래부터 손실이 크다.
3. **AI 세션 (신규)** — 프롬프트, 계획, 기각된 시도, 에이전트가 탐색하며 파악한 코드베이스 지식. **세션 종료와 함께 증발하고, 팀의 누구와도 공유되지 않는다.**

AI 도구가 만든 본질적 변화: 3번 저장소에 쌓이는 맥락의 양이 폭증했다. 과거에는 머릿속에만 있던 사고 과정이 이제 전부 텍스트로 존재한다. 그런데 이것을 팀으로 흘려보내는 파이프라인이 없다.

**역설: 개발 맥락이 역사상 가장 잘 "기록"되는 시대에, 맥락 "공유"는 가장 안 되고 있다.**

### 1.3 AI가 협업 문제를 증폭시키는 6가지 메커니즘

1. **속도 증폭** — 에이전트가 대량 diff를 빠르게 생산 → 팀원들이 아는 코드베이스 상태가 더 빨리 낡음 → 충돌 비용이 초선형으로 증가.
2. **맥락 비대칭** — 내 에이전트는 내가 말해준 것만 안다. 결과: 컴파일은 되지만 의미적으로 충돌하는 코드(중복 유틸리티, 상반된 아키텍처 방향).
3. **결정 불투명** — "AI가 왜 이렇게 짰지?" 20턴짜리 협상 대화는 증발하고 PR에는 결과만 남는다.
4. **중복 탐색** — 개발자 A의 에이전트가 이미 알아낸 사실을 B의 에이전트가 토큰을 써가며 재발견.
5. **규약 표류 (context drift)** — 각자의 rules/memories가 달라 AI 생성 코드 스타일이 사람 수만큼 갈라짐.
6. **보이지 않는 WIP** — 에이전트가 브랜치/워크트리에서 몇 시간씩 작업. presence 신호가 없어 정면충돌.

### 1.4 문제가 실재한다는 근거 (2026년 기준)

- **CooperBench** (Stanford+SAP Labs, arXiv 2601.13295, 2026-01): 협업 태스크 600+개 벤치마크. 에이전트 2개가 협업하면 단독 대비 성공률 **약 30% 하락**, 최상위 모델 페어도 협업 성공률 ~25%. 통신 채널을 줘도 거의 개선 안 됨. 실패 3유형: 모호한 메시징, 약속 이탈, 상대에 대한 잘못된 믿음.
- **Stanford HAI** "AI Coding Agents Fail at Teamwork" (2026-06): 에이전트들이 서로의 머지 컨플릭트 경고를 무시하는 "coordination gap".
- **AgenticFlict** (arXiv 2604.03551, 2026): 59K+ 리포의 에이전트 작성 PR 142K+건에서 발생한 머지 컨플릭트 대규모 데이터셋.
- **Cursor의 후퇴**: 자동 Memories를 출시(2025 중반)했다가 2.1.x에서 제거하고 git 리뷰를 거치는 rules로 회귀 — "검토되지 않은 학습 컨텍스트를 팀이 신뢰하지 않는다"는 가장 강한 시장 신호.
- **미충족 수요 (오픈 이슈들)**: Claude Code 팀 공유 메모리 FR(anthropics/claude-code#38536), 세션 공유(#40981), 실시간 멀티유저 세션(#60082), Codex 세션 공유(openai/codex#13251), Cursor 포럼의 채팅 공유 요청 다수. 3대 로컬 에이전트 모두 미해결.
- **한국 현장 사례**: GPTers 커뮤니티 "바이브코더가 여러 명이 되면 생기는 일" — 다인원 바이브코딩 팀에서 실험 중복과 600+건 데이터 불일치 발생, "어느 데이터를 믿어야 하나" 붕괴 후 단일 진실원천 구조를 수동으로 재구축한 field report.
- **업계 담론**: Aviator "multiplayer AI coding" 테제(2026-02), Augment "조율되지 않은 병렬 에이전트는 중복 구현과 의미적 모순을 만든다", Packmind가 명명한 "context drift", Simon Willison·Pragmatic Engineer의 병렬 에이전트 워크플로 확산 관찰.

---

## 2. 해결 방향 — 6레이어 모델

문제를 한 방에 푸는 단일 해법은 없고, 여섯 개 레이어가 쌓인다. 레이어별로 2026년 현재 "누가 채웠는지"가 극명하게 갈린다.

| 레이어 | 해법 | 2026-07 현재 상태 |
|---|---|---|
| **L0 프로세스** | 트렁크 기반, 작은 PR, 짧은 사이클로 분기 시간 자체를 축소 | 도구가 아닌 운영. 필요조건일 뿐 충분조건 아님 |
| **L1 리포 = 맥락 저장소** | AGENTS.md/CLAUDE.md, rules, ADR, 스펙을 커밋 | **표준화 완료 구역.** AGENTS.md 6만+ 리포, Linux Foundation 표준. 단 정적·수동 |
| **L2 자동 캡처·증류** | 세션에서 결정·학습을 **부산물로** 추출 | 개인용만 성숙(claude-mem 8.8만 스타). 팀용은 태동기 |
| **L3 팀 공유 메모리** | 팀 메모리 서버(MCP), git-backed 메모리 | 초기 단계. 인디·소규모 OSS뿐, 승자 없음 |
| **L4 실시간 조정** | presence, 태스크 클레이밍, 충돌 조기경보 | **거의 완전한 공백.** Beads의 클레이밍, GitHub 연구 프로토타입 정도 |
| **L5 의도의 산출물화** | 스펙·작업 그래프를 진실원천으로 승격 | 폭발적 채택(Spec Kit 12.4만 스타) — 단, 대부분 싱글플레이어 |
| **L6 조직 컨텍스트 엔진** | 코드+문서+슬랙 인덱싱 → 모든 에이전트에 서빙 | 혼잡한 엔터프라이즈 시장. sales-led, 50석+ 지향 |

### 조사에서 도출된 핵심 통찰 3가지

1. **사람에게 문서화를 시키는 해법은 실패한다.** 맥락은 AI 세션 안에 이미 텍스트로 존재한다. 캡처는 zero-effort 부산물이어야 한다. (수동 큐레이션 방식 — Copilot Spaces, Devin Knowledge — 은 "쓰는 사람만 쓰는" 문서 문제를 그대로 재생산)
2. **학습된 컨텍스트는 리뷰 없이는 신뢰받지 못한다.** Cursor가 auto-Memories를 제거하고 git 리뷰 rules로 회귀한 것이 증거. 즉 "메모리에도 PR이 필요하다" — 개인 메모리 → 팀 메모리 승격에 리뷰·머지·충돌해소 시맨틱스가 필요한데, 이걸 만든 제품이 없다.
3. **사후 공유만으로는 충돌을 못 막는다.** 커밋·PR·문서는 전부 과거형이다. 중복 작업과 의미적 충돌을 실제로 막으려면 **진행 중 의도(in-flight intent)** 의 실시간 교환이 필요하다.

---

## 3. 기존 프로젝트 지도 (2026-07-27 검증)

### 3.1 정적 규칙·지침 — 표준화가 끝난 구역

| 프로젝트 | 내용 | 상태 |
|---|---|---|
| **AGENTS.md** (agents.md) | "에이전트용 README" 단일 파일 표준. OpenAI 주도 → 2025-12-09 Linux Foundation AAIF 기증 | 6만+ 리포, ~28개 도구 네이티브 지원. **단 Claude Code는 미지원 홀드아웃**(이슈 3,200+ 리액션, symlink 워크어라운드) |
| **CLAUDE.md / .cursor/rules / copilot-instructions** | 각 도구의 리포 커밋형 지침 | 사실상 업계 기본기 |
| **Ruler** (intellectronica) | `.ruler/` 단일 소스 → 30+ 도구 네이티브 설정 생성 | 2.8k 스타, 활발. 유사: rulesync |
| **Packmind** | 표준·ADR을 조직 거버넌스로 관리, 8개 도구에 배포. "context drift" 방지가 명시 목표 | OSS+유료, 활동 중 |

### 3.2 팀 공유 메모리 레이어

| 프로젝트 | 내용 | 팀 공유 방식 | 상태 |
|---|---|---|---|
| **ByteRover / Cipher** | 코딩 에이전트용 메모리를 git처럼 branch/commit/merge/push/pull하는 "context tree". "팀 전체 코딩 에이전트의 중앙 메모리 레이어" — 가장 직접적인 시도 | 클라우드 워크스페이스 + MCP(22+ 도구) | Cipher OSS 2025-08 → byterover-cli로 개명, ~4.9k 스타. Free/Pro $15 |
| **Supermemory** | 범용 메모리 API + Claude Code 플러그인(2026-01) | container tag로 개인/팀 메모리 분리, 클라우드 공유 | $2.6M 시드, 초기 |
| **cognee** | 지식그래프+벡터 메모리 엔진 | 서버 모드에서 팀이 그래프 하나를 공유, Team $200/월(10인) | $7.5M 시드(2026-02), 28k 스타 |
| **Context Cloud** | "엔지니어링 팀용 공유 AI 메모리 서버" — 팀 워크스페이스, RBAC, 메모리별 작성자 표시 | MCP (모든 클라이언트) | 인디, 매우 초기 |
| **Letta (MemGPT)** | Letta Code로 코딩 피벗(2026-03). "Context Repositories" — 메모리를 git 버전관리 파일로 | git 파일이므로 공유 가능하나 팀 워크플로 미출시 | 23k 스타, $10M |
| **Devin Knowledge** (Cognition) | 조직 스코프 지식 항목, 전 세션 자동 적용 | Devin 전용 (락인) | 성숙, 상용 |
| *(대조: 개인용만)* | **claude-mem**(8.8만 스타, 세션 자동 캡처·압축·재주입 — 개인 전용), Mem0/OpenMemory(개인), Pieces(개인), Memorix(크로스툴이지만 개인) | — | 개인용은 대성공, 팀용은 전부 미약 |

### 3.3 세션 캡처·공유·프로버넌스

| 프로젝트 | 내용 | 상태 |
|---|---|---|
| **Amp** (Sourcegraph에서 2025-12 분사) | 모든 에이전트 세션(추론·툴콜·수정 전체)이 워크스페이스에 **기본 공유**. 리뷰에 스레드 링크 첨부 문화. 유일한 default-on 팀 가시성 | 상용, 활발. 단 Amp 전용(범용 export 없음) |
| **SpecStory** | Cursor/Claude Code/Codex 등 세션을 로컬 마크다운으로 자동 저장. `.specstory/history/` 커밋으로 팀 가시화 가능 | 클라우드는 아직 싱글유저, 팀 기능 "coming soon". 세션→스킬 변환 OSS "Lore" |
| **Git AI** (git-ai-project) | **라인 단위 프로버넌스**: 어느 에이전트/모델/프롬프트/세션이 각 라인을 작성했는지 git notes에 기록(rebase 생존). 12+ 에이전트 훅 | 2.4k 스타, 1년 미만 179 릴리스, Thoughtworks Radar 등재. 크로스툴 프로버넌스 표준에 최근접 |
| **GitHub Copilot** | 코딩 에이전트 세션 로그를 PR 열람자 누구나 재생("View session"). Copilot Spaces(팀 컨텍스트 번들, GA 2025-09) | 성숙. Spaces는 수동 큐레이션 |
| **CodeRabbit Learnings** | PR 코멘트의 자연어 선호를 자동 축적, org 전체 공유 리뷰 메모리 | 성숙하나 리뷰 스코프·자사 전용 |
| **CCPM** (automazeio) | PRD→에픽→태스크를 GitHub Issues에 동기화, 에이전트가 이슈 코멘트로 진행상황 게시 → 팀 실시간 가시화·핸드오프 | 8.3k 스타 OSS |
| **CodeAlmanac** (YC S26) | **팀의 Claude Code/Codex 대화 트랜스크립트에서** 결정·불변조건·gotcha를 추출해 리포 내 `almanac/` 위키 자동 구축 | **Show HN 2026-07-21** (이번 주!), 668 스타, macOS 전용. 자동증류×팀공유에 가장 근접한 신생 |
| **Dosu** | 코딩 에이전트 세션 중 백그라운드로 팀 지식베이스 자동 축적, MCP로 재공급 | 상용, OSS 메인테이너층 인기 |
| *(대조)* | Cursor·Claude Code·Codex CLI 모두 **세션 공유 자체가 미해결 오픈 이슈**. Claude Code의 신기능은 live Artifacts(사람용 페이지)뿐 | — |

### 3.4 작업 조정 — 태스크·스펙·오케스트레이션

| 프로젝트 | 내용 | 멀티 개발자? | 상태 |
|---|---|---|---|
| **Beads** (Steve Yegge) | git-backed 의존성 그래프 이슈 트래커 = 에이전트의 영속 공유 작업 메모리. v1.0에서 Dolt 백엔드(셀 단위 머지, **원자적 태스크 클레이밍**, 동시 쓰기) | **예 — 진짜 멀티휴먼+멀티에이전트 가능한 거의 유일한 git-backed 트래커** | 2025-10 출시 → 25.7k 스타 |
| **GitHub Spec Kit** | constitution/스펙/계획/태스크 커밋형 스펙 주도 개발, 30+ 에이전트 | 산출물은 팀 리뷰 가능, 조정 기능 없음 | **~124k 스타** — 이 분야 최대 채택 |
| **OpenSpec** (Fission-AI) | 경량 스펙 주도 + **"Stores" 베타: git 동기화 전용 플래닝 리포로 멀티팀 요구사항 공유** | Stores가 명시적 멀티팀 | 62.7k 스타 |
| **Task Master** | PRD→구조화 태스크(.taskmaster/) | 싱글플레이어(락/클레임 없음) | 27.9k 스타 |
| **Backlog.md** | 태스크=마크다운, 터미널/웹 칸반 | git 가시성만, 충돌 방지 없음 | 6.3k 스타 |
| **AWS Kiro** | 스펙 주도 IDE. `.kiro/specs/`+steering 커밋 = git 통한 팀 공유 의도 | 스펙은 팀 공유, 실행은 개인 | GA 2026 (활발) |
| *(1인용 오케스트레이터)* | Conductor, claude-squad(7.9k), Gas Town(15.9k, Beads 기반 20~30 에이전트 도시), Emdash(YC W26), HumanLayer CodeLayer, Runtime(YC S26) | 전부 **한 사람의 N 에이전트** 조정 | 활발 |
| *(사망/피벗)* | **Terragon 셧다운(2026-01-16)**, **Vibe Kanban/Bloop 셧다운(2026-04-10**, "비즈니스 모델 부재"**)**, **Tessl 피벗(2026-01-29**, 스펙→스킬 레지스트리**)** | — | 카테고리 상업성 경고 신호 |

### 3.5 조직 컨텍스트 엔진 (엔터프라이즈)

| 프로젝트 | 한 줄 | 상태 |
|---|---|---|
| **Sourcegraph** | 멀티리포 코드 그래프 + MCP 서버 + Deep Search. Cody 개인용 종료(2025-07), Amp 분사(2025-12) | 엔터프라이즈 ~$16k/년~ |
| **Augment Code** | 팀 단위 실시간 Context Engine (40~50만 파일). 2026 Cosmos/Intent로 오케스트레이션 확장 | ~$270M 조달 |
| **Unblocked** | 코드+PR+Slack+Jira+Notion 통합 인덱스 → 단일 MCP로 Claude Code/Cursor 등에 공급 | $29/인/월 |
| **Factory** | org 컨텍스트를 가진 엔터프라이즈 Droids. AGENTS.md 공동 저자 | Series C $150M(2026-04), $1.5B |
| **Qodo Aware** | 멀티리포+PR 히스토리 컨텍스트 엔진 → 리뷰·생성 에이전트에 공급 | Qodo 2.0 (2026-02) |
| **Tabnine** | 엔터프라이즈 컨텍스트 엔진, 에어갭 배포 | 엔터프라이즈 전용화 |
| **Zencoder** | Repo Grokking + 조직 공유 커스텀 에이전트(Zen Agents) | 활동 중 |
| **Greptile / Nia / Swimm / DeepWiki** | 전체 코드베이스 그래프 리뷰($25M A) / 에이전트용 컨텍스트 API(YC S25) / 문서→AI 컨텍스트(모더나이제이션 피벗) / 자동 리포 위키+MCP(3만+ 공개 리포) | 각기 활동 중 |

**공통 한계**: 전부 **이미 존재하는 산출물**(커밋된 코드, 닫힌 PR, 문서, 슬랙)을 인덱싱한다. PR이 생기기 전, 각 개발자의 에이전트가 지금 하고 있는 것은 아무도 모른다.

### 3.6 플랫폼 인컴번트의 직접 진입 신호

- **GitHub Agent HQ** (2025-10 발표): 서드파티 에이전트들을 GitHub에서 할당·조종·추적하는 "mission control". org 컨트롤 플레인.
- **GitHub Next "Ace"** (기술 프리뷰, 수천 명): **명시적 멀티플레이어 에이전트 협업 환경** — Slack형 공유 세션, 팀원이 서로의 세션에 들어가 프롬프트 히스토리를 보고, 스펙을 공동 편집하고, 구현 전에 에이전트 계획을 리뷰. **이 문제에 대한 가장 직접적인 공격.** 단 클라우드 플랫폼 네이티브(리포 네이티브 아님).
- **Claude Code**: Agent Teams(1인의 에이전트 팀이지만 공유 태스크리스트·메일박스·파일 락 개념 등장), live shareable Artifacts(2026-06 베타). 팀 메모리 FR(#38536)이 열려 있음 — 언제든 네이티브로 흡수 가능.

### 3.7 연구·벤치마크 (문제의 학술적 검증)

- **CooperBench** — "The Curse of Coordination". 협업 시 성능 30% 붕괴 정량화.
- **AgenticFlict** — 에이전트 PR 머지 컨플릭트 142K건 데이터셋.
- **MetaGPT / ChatDev** — 에이전트-only 소프트웨어 팀 시뮬레이션(사람 간 공유 문제는 다루지 않음, 인접 분야).
- "Evaluating AGENTS.md" (arXiv 2602.11988) — 공유 컨텍스트 파일의 실효성 실증 연구.

### 3.8 한국 생태계 — 담론은 있고 제품은 없다

- **박재홍, "컨텍스트가 곧 코드다"** (wikidocs) — 이 문제에 대한 가장 직접적인 한국어 에세이. 컨텍스트를 리포에 체크인하고, 라이브러리처럼 패키징하고, 레지스트리를 만들자는 제안. "한 사람이 컨텍스트를 고치면 팀 전체가 물려받는다".
- **GPTers 커뮤니티 사례** — 다인원 바이브코딩 붕괴와 수습의 field report (1.4절 참조).
- **기업 내부 실천**: LY Corp(Claude Code Action을 조직 코드리뷰 플랫폼으로), Toss(AI Surf Day, 전사 AI 도구 배포), 당근(KAMP 내부 에이전트 플랫폼 — 단 개발 컨텍스트 공유용은 아님), 여기어때(팀 공통 커맨드 + Claude Code Agent Teams 표준화), 한컴(CLAUDE.md 관리 전략 발신).
- **결론: 국내 전용 제품은 전무.** 채택과 담론은 활발("바이브코딩 협업 도구도 나오려나요?" 같은 수요 발화 존재)한데 제품이 없는 공백 시장.

---

## 4. 갭 분석 — 아무도 채우지 못한 4개의 빈 사분면

40여 개 프로젝트를 배치해 보면 공백이 명확하다.

1. **자동 캡처 × 팀 공유 = 빈 사분면.**
   자동 캡처 제품은 전부 개인용(claude-mem 8.8만 스타, OpenMemory, Pieces), 팀 공유 제품은 전부 수동 큐레이션(Devin Knowledge, Copilot Spaces)이거나 원시 로그(Amp 스레드). 각 개발자의 에이전트가 배운 것을 자동 증류해 팀원에게 귀속 표시와 함께 발행하는 주류 제품이 없다. 시도들은 전부 신생·소규모(ByteRover, CodeAlmanac, Dosu, Context Cloud).

2. **크로스툴 × 크로스팀 = 빈 사분면.**
   크로스툴 메모리는 개인 전용(Memorix, claude-mem), 팀 메모리는 단일 벤더 전용(Devin, Amp, Copilot, CodeRabbit). Claude Code+Cursor+Copilot을 섞어 쓰는 현실의 팀에게는 git에 커밋된 마크다운(AGENTS.md/CLAUDE.md) 말고는 공유 레이어가 아예 없다.

3. **과거 공유는 있지만, 라이브 의도 공유는 없다.**
   모든 시스템이 사후(retrospective) 지식을 공유한다. 정작 중복 작업과 의미 충돌을 막아줄 "지금 누구의 에이전트가 무엇을 계획하고 어떤 파일을 만지는 중인가"는 어떤 리포 네이티브 표준에도 없다. 근접 시도: Beads의 원자적 태스크 클레이밍, GitHub Next Ace(클라우드 프리뷰), Amp 스레드(원시·자사 전용).

4. **메모리의 리뷰·머지 시맨틱스 부재.**
   Cursor의 auto-Memories 철회가 보여주듯 팀은 검토 안 된 학습 컨텍스트를 신뢰하지 않는다. 그런데 메모리를 코드처럼 다루는 — PR로 제안되고, 리뷰되고, 머지되고, 두 에이전트가 모순되게 배웠을 때 충돌 해소되는 — 제품이 없다. Letta의 git-backed MemFS와 ByteRover의 context tree가 기반 기술은 갖췄지만 멀티 개발자 리뷰 워크플로가 없다.

**구조적 관찰 4가지:**
- **규칙 레이어는 표준화됐지만 작업 레이어는 상호운용 제로.** Beads 그래프·Backlog.md·Task Master JSON·Spec Kit·OpenSpec 레이아웃이 전부 비호환.
- **프로버넌스와 재사용의 단절.** Git AI가 "어느 프롬프트가 이 라인을 만들었나"를 기록하지만, 팀원이 같은 코드를 만질 때 그 맥락("이 함수는 세션 X에서 제약 Y 하에 생성됨")을 라이브로 되먹이는 도구는 없다. 캡처·가시성·재사용이 서로 다른 제품에 흩어져 있고 그 이음새가 공백.
- **2~10인 팀 공백.** 엔터프라이즈 엔진은 sales-led($16k/년~) 또는 50석+ 지향. 병렬 AI 사용과 머지 고통이 가장 격렬한 소규모 팀에게는 미성숙 OSS뿐.
- **카테고리 상업성 경고.** Terragon·Vibe Kanban 1년 내 사망, Tessl 피벗. 반면 무료 리포 네이티브 OSS는 폭발(Spec Kit 124k). 교훈: 포맷은 무료·리포 네이티브로 채택시키고, 수익화는 팀 동기화/거버넌스에서.

---

## 5. 기회 가설 — coopera가 노릴 수 있는 지점

> 가설 단계. `01-idea-brief.md`(idea-refiner)에서 타깃·범위를 검증할 것.

### 5.1 포지셔닝 한 줄

**"자동 캡처되고(zero-effort) · 팀에 공유되고(reviewed) · 도구를 가리지 않고(MCP/git) · 실시간인(presence) 개발 맥락 레이어"** — 위 4개 빈 사분면의 교집합.

### 5.2 제품 파이프라인 스케치 (5단)

1. **캡처 (자동)** — 각 개발자 도구에 훅(Claude Code hooks, Cursor/Codex 래퍼) → 세션에서 {의도, 계획, 결정, 학습, 접촉 파일} 이벤트 추출. 사용자 행동 변화 요구 0.
2. **증류** — LLM이 이벤트를 코드 엔티티(파일/심볼/PR)에 앵커된 구조화 "컨텍스트 이벤트"로 압축. 신뢰도 점수 + TTL(낡음 관리).
3. **공유 (신뢰 게이트)** — 개인 메모리 → 팀 메모리 승격에 PR형 리뷰. 저장은 git-native(브랜치/notes 또는 `.coopera/` 디렉터리)로 시작해 서버 없이 도입 가능하게, 실시간성은 경량 동기화 서버로 보강.
4. **라이브 조정** — presence 브로드캐스트("A의 에이전트가 payments/ 리팩터링 중, 태스크 클레임됨") + **충돌 레이더**: 활성 에이전트들의 계획·접촉 파일을 비교해 머지 전에 경보.
5. **소비** — ① MCP 서버로 모든 팀원의 에이전트가 세션 시작 시 관련 맥락 자동 주입 ② PR에 맥락 주석("이 코드는 제약 Y 하에 생성됨") ③ 사람용 다이제스트(Slack/CLI).

### 5.3 MVP 웨지 제안

- **도구**: Claude Code 우선 (hooks·MCP·플러그인이 네이티브, 팀 메모리 FR이 열려 있는 미충족 수요, 한국 시장에서 Claude Code 채택 강세 — Toss·당근·컬리) → 이후 Cursor/Codex로 확장.
- **팀 크기**: 2~10인 (가장 아픈데 아무도 안 팜).
- **아키텍처**: git-backed로 시작(도입 마찰 0, 셀프호스트 신뢰) + 선택적 동기화 서버(실시간 presence).
- **첫 데모 시나리오**: 개발자 A가 오전에 결정한 내용을, 오후에 개발자 B의 에이전트가 같은 모듈을 건드리려 할 때 자동으로 주입받아 중복/충돌을 회피하는 것을 보여주기.

### 5.4 최근접 경쟁과의 차별화

| 대비 | 그들 | coopera 차별점 |
|---|---|---|
| Amp | 세션 기본 공유하지만 원시 스레드·자사 에이전트 전용 | 크로스툴 + 증류된 이벤트 + 리뷰 게이트 |
| ByteRover | 팀 메모리 + git형 버전관리 (가장 유사) | 라이브 조정(presence·충돌 레이더) 없음, 리뷰 워크플로 없음 |
| CodeAlmanac | 트랜스크립트→위키 (사후 문서) | 위키가 아닌 이벤트 스트림 + 실시간 + 크로스 개발자 |
| Beads | 태스크 그래프 공유·클레이밍 | 태스크만이 아닌 맥락 5종 전체; Beads와는 통합 후보 |
| GitHub Ace | 멀티플레이어 세션 (가장 위협적) | 리포 네이티브·도구 불문·플랫폼 락인 없음 |

### 5.5 리스크

1. **플랫폼 리스크 (최대)** — GitHub(Ace/Agent HQ), Anthropic(#38536 네이티브화), Cursor가 자사 생태계에 기본 탑재하면 독립 제품 공간이 좁아짐. 방어선 = 크로스툴 중립성. 단 이는 시간 창이 좁다는 뜻이기도 함.
2. **신호/잡음 및 신뢰** — 자동 캡처의 고질병. Cursor 후퇴의 교훈대로 리뷰 게이트가 해답이지만, 리뷰 자체가 마찰이 되는 딜레마를 UX로 풀어야 함.
3. **상업성** — 이 카테고리에서 2개사 사망. 무료 OSS 코어(포맷·캡처) + 유료 팀 동기화/거버넌스 모델이 현실적.
4. **프라이버시** — 프롬프트에는 비밀·감정·실수가 담김. 레드액션과 공유 범위 제어 필수(Git AI가 프롬프트 본문을 git 밖 별도 저장소에 두는 선례).

---

## 6. 주요 출처

- 표준·재단: agents.md · Linux Foundation AAIF 발표(2025-12-09)
- 메모리: github.com/campfirein/byterover-cli · mem0.ai/openmemory · cognee.ai · getzep.com · letta.com/blog/context-repositories · github.com/thedotmack/claude-mem · supermemory.ai · contextcloud.pro
- 세션·프로버넌스: specstory.com · ampcode.com/manual · github.com/git-ai-project/git-ai · docs.devin.ai/product-guides/knowledge · deepwiki.com · docs.coderabbit.ai/knowledge-base/learnings · github.com/automazeio/ccpm · github.com/AlmanacCode/codealmanac
- 리포 네이티브: github.com/steveyegge/beads · github.com/github/spec-kit · github.com/Fission-AI/OpenSpec · github.com/eyaltoledano/claude-task-master · github.com/MrLesk/Backlog.md · kiro.dev · github.com/intellectronica/ruler · block.github.io/goose/docs/guides/recipes
- 엔진·시장: sourcegraph.com · augmentcode.com · getunblocked.com · factory.ai · qodo.ai · zencoder.ai · dosu.dev · greptile.com · trynia.ai · packmind.com · github.blog(Agent HQ, Copilot Spaces GA) · githubnext.com/talks/one-developer-two-dozen-agents-zero-alignment
- 연구: cooperbench.com (arXiv 2601.13295) · hai.stanford.edu "AI Coding Agents Fail at Teamwork" · arXiv 2604.03551 (AgenticFlict) · arXiv 2602.11988 (AGENTS.md 평가)
- 한국: wikidocs.net/blog/@jaehong/12933 · gpters.org(바이브코더 협업 사례) · techblog.lycorp.co.jp/ko(Claude Code Action 플랫폼화) · toss.tech(AI Surf Day) · byline.network(당근 KAMP) · techblog.gccompany.co.kr(여기어때 Agent Teams)

미해결 오픈 이슈(수요 증거): anthropics/claude-code #38536(팀 메모리), #40981(세션 공유), #60082(멀티유저 세션), #6235(AGENTS.md 지원) · openai/codex #13251(세션 공유)
