# 04 — 스캐폴드 리포트

- 수행일: 2026-07-28 · 파이프라인 최종 단계 (01 브리프 → 02 기획 → 03 PRD → **04 스캐폴딩 완료**)
- 검증 상태: **build ✓ · 유닛 테스트 15/15 ✓ · 스모크(M1 수직 슬라이스 e2e) 1/1 ✓ · fmt ✓**

## 생성 내역

```
Cargo.toml                      # cargo workspace (resolver 2)
crates/coopera-core/            # lib — 엔진 로직 (모듈별 단위 테스트 인라인)
  src/config.rs                 #   .coopera/config.toml 로드, 토큰 예산(1500/300/800/200)
  src/gitio.rs                  #   시스템 git 셸아웃 래퍼 (discover/branch/changed_files)
  src/wiki.rs                   #   페이지 모델·frontmatter 파서·F4 린트 규칙
  src/inject.rs                 #   F2 주입 팩 빌더 — anchor 랭킹 + 하드 예산 + 스파이크 검증 지시문
  src/hookio.rs                 #   훅 stdin/stdout JSON 프로토콜 (관용 파싱 = fail-open)
  src/digest.rs                 #   F3 다이제스트 고정 스키마 (≤30줄 렌더)
  src/redact.rs                 #   시크릿 레드액션 (발행 전 필수)
crates/coopera-cli/             # bin "coopera" — 얇은 명령 레이어
  src/main.rs                   #   clap: init | hook session-start|session-end | distill | wiki lint | status
  src/cmd_init.rs               #   F1 — 구현 완료 (멱등)
  src/cmd_hook.rs               #   F2 구현 완료 / F3 spawn 배선 완료
  src/cmd_distill.rs            #   F3 골격 — TODO(F3) 증류기 본체
  src/cmd_wiki.rs               #   F4 — 구현 완료
  tests/smoke.rs                #   M1 수직 슬라이스 e2e
CLAUDE.md · README.md · .github/workflows/ci.yml (fmt+build+test)
```

- 공식 CLI 사용 여부: cargo 워크스페이스는 수동 구성(관례 그대로). 폴백·이탈 없음.
- 스파이크 마커 파일(`.codex/config.toml`)은 남겨둠 — M2의 F8(Codex 어댑터)에서 실제 훅으로 교체 예정.

## 검증 결과 (실행한 명령)

| 명령 | 결과 |
|---|---|
| `cargo build` | 성공 (1회차, 19.7s) |
| `cargo test` | core 유닛 15/15 통과 |
| `cargo test -p coopera-cli` | 스모크 1/1 통과 (0.5s) |
| `cargo fmt --check` | 정리 후 통과 |

## 워킹 슬라이스가 증명한 것 (F2 end-to-end)

스모크 테스트가 임시 리포에서 실제 바이너리로 검증한 흐름:

1. `git init` → `coopera init` → wiki/·.coopera/·.claude/settings.json(훅 병합)·AGENTS.md(마커 블록) 생성, **재실행 시 no-op(멱등)**
2. 결정 페이지(`anchors: src/payments/`) + 매칭 변경 파일 생성 → `hook session-start`에 `{}` stdin
3. 출력 JSON의 `additionalContext`에 결정 summary와 Guidance(스파이크 ② 검증 문구) 포함, `systemMessage`에 "coopera: injected N items" 가시화 — **PRD의 아키텍처(훅→엔진→wiki→예산→주입)가 실제로 성립**
4. 리포 밖에서 실행 → exit 0 + "inactive" (fail-open)
5. `wiki lint` — 정상 위키 통과, 스키마 위반 페이지에서 비0 종료

## 미해결 질문의 최종 상태

| 항목 | 상태 |
|---|---|
| 증류 품질 | F3 본체 구현 후 실세션 리뷰로 판정 (CLAUDE.md 체크리스트로 이관) |
| SessionEnd transcript_path 형식 | F3 구현 중 실측 (이관) |
| Codex apply_patch 훅 발화 | M2 실측 (이관) |
| stale 주입 제외 | last_verified 비교 미구현 — M1 잔여 작업으로 체크리스트에 명시 |

## 다음 할 일 = M1 잔여 (CLAUDE.md 체크리스트)

**F3 증류기 본체**가 유일한 큰 덩어리다: `claude -p` 호출 → Digest 스키마 파싱 → `wiki/sessions/` 기록 → 위키 diff 스테이징 → 레드액션. 완료되면 이 리포에 `coopera init`을 실행해 **self-hosting 도그푸딩 시작**(M1 완성 기준: 실세션 1회전 후 위키 diff를 사용자가 리뷰).

파이프라인은 여기서 끝난다 — 이후는 CLAUDE.md와 PRD M1 체크리스트만으로 개발을 이어간다.
