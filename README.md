# tos

| 항목 | 값 |
|------|----|
| 생성일 | 2026-06-07 |
| 프로젝트 모드 | Reverse |
| 팀 구성 | AGI 개발팀 7명 (PM + Architect + DeveloperBE/FE + DevOps + Designer + QA) |
| 생성 도구 | AgiTeamBuilder make |

---

## 🚀 시작하기

```bash
cd /Users/patrickukim/Projects/tos
./agiteam.sh
```

## 🧠 MemPalace hook

`.claude/settings.local.json`은 생성 시점의 `mempalace` 실행 경로를 사용합니다.
필요 시 먼저 `uv tool install mempalace`를 실행하세요.

## 📂 구조

```
tos/
├── brain/                  🌐 7역할 페르소나 인스턴스 (git 추적)
├── documents/              🌐 산출물 (01.proposal / 02.reverse / 03.management / 04.development / 05.operation)
├── system/                 🌐 소스코드 (GitHub repo 단위 하위 폴더)
├── .claude/                🔒 Claude Code hook + MemPalace autosave
├── agiteam.sh              팀 부팅 스크립트
├── agiteam.json            팀 구성·설정
├── project_state.yaml      프로젝트 상태·모드 메타
└── README.md               본 문서
```

## 🔗 핵심 규약

- **버전**: 마이너(v0.2 → v0.3)는 에이전트 자율, **메이저(v1.0, v2.0)는 유저 명시 승인 시에만**
- **업무-산출물**: 일을 했다 = 산출물을 작성했다. 아웃풋 없으면 일 안 한 것
- **작업 근거**: 프로젝트 모드(WBS) / 운영 모드(레드마인 이슈)
- **착수 제어**: PM 지시 없이 자발 착수 금지, 선행 산출물 없으면 대기
- 상세 원칙은 `brain/PM/persona.md` (원형 상속) 참조

---

_2026-06-07 AgiTeamBuilder로 스카폴딩됨_
