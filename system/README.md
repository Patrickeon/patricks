# system/

> **실행 자산 저장 공간** — 소스코드가 들어가는 폴더.
>
> 🌐 고객 공개 영역 (납품 대상)

---

## 저장 규칙

**이 폴더 아래에는 GitHub 리포지토리 단위로 하위 폴더를 만들어 쓴다.**

즉, `system/`의 직계 하위 폴더 한 개 = 독립된 GitHub 리포지토리 한 개.

각 하위 폴더는 자체 `.git/`을 가지며, AgiTeamBuilder(또는 파생 프로젝트)의 루트 git과는 별개로 관리된다.

## 예시 구조

```
system/
├── README.md                 ← 본 문서
├── <repo-1>/                 ← GitHub 리포 1 (예: backend)
│   ├── .git/
│   └── ...
├── <repo-2>/                 ← GitHub 리포 2 (예: frontend)
│   ├── .git/
│   └── ...
└── <repo-3>/                 ← GitHub 리포 3 (예: infra)
    ├── .git/
    └── ...
```

## 템플릿 상태

본 `AgiTeamBuilder/system/`은 **빈 껍질**이다. 실제 리포지토리는 각 프로젝트에서 `make` 실행 후 개별 `git clone` 또는 `git init`으로 채운다.

## 상위 프로젝트 루트의 git과의 관계

- AgiTeamBuilder/파생 프로젝트의 **루트 git**: `documents/·brain/·system/README.md` 등 메타 구조만 추적
- **`system/<repo-N>/`**: 각자 자체 git으로 관리. 루트 git이 이 하위를 추적하지 않도록 `.gitignore`에 제외 또는 **git submodule**로 연결
- 관리 방식(서브모듈 vs 무시)은 각 프로젝트가 DevOps와 상의해서 선택

---

*본 폴더는 AgiTeamBuilder 템플릿에 포함되며, `make`으로 새 프로젝트 생성 시 그대로 복사된다.*
