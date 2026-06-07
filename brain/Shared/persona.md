# Shared 페르소나 (공용 레퍼런스)

> 모든 역할이 세션 부팅 시 참조하는 **공용 상수** 모음.
> 변동 없는 상수만 기록 — URL·API 사용법·트래커/상태 ID 등.
> 프로젝트별 가변 값(project_id·개별 API 키·role user_id 등)은 각 역할 인스턴스(`persona.md`)에 기록한다.

---

## 1. 레드마인 공용 접속 정보

- **URL**: `http://211.117.60.5:8080/`
- **프로토콜**: HTTP (내부망)
- **인증**: `X-Redmine-API-Key` 헤더 (각 역할별 개인 키)

---

## 2. 레드마인 API 사용법

### 2.1 이슈 생성 (새기능)

```bash
curl -s -X POST -H "Content-Type: application/json" \
  -H "X-Redmine-API-Key: <API_KEY>" \
  -d '{
    "issue": {
      "project_id": "<PROJECT_ID>",
      "tracker_id": 2,
      "subject": "제목",
      "description": "내용",
      "assigned_to_id": <USER_ID>
    }
  }' \
  http://211.117.60.5:8080/issues.json
```

### 2.2 결함 등록

```bash
curl -s -X POST -H "Content-Type: application/json" \
  -H "X-Redmine-API-Key: <API_KEY>" \
  -d '{
    "issue": {
      "project_id": "<PROJECT_ID>",
      "tracker_id": 1,
      "subject": "[결함] 제목",
      "description": "재현 절차 / 기대 결과 / 실제 결과 / 심각도"
    }
  }' \
  http://211.117.60.5:8080/issues.json
```

### 2.3 진척률 업데이트 + 상태 전이

```bash
# 해결 보고 (작업자)
curl -s -X PUT -H "Content-Type: application/json" \
  -H "X-Redmine-API-Key: <API_KEY>" \
  -d '{"issue":{"done_ratio":100,"status_id":3}}' \
  http://211.117.60.5:8080/issues/<ISSUE_ID>.json

# 최종 완료 처리 (PM 검토 후)
curl -s -X PUT -H "Content-Type: application/json" \
  -H "X-Redmine-API-Key: <API_KEY>" \
  -d '{"issue":{"status_id":5}}' \
  http://211.117.60.5:8080/issues/<ISSUE_ID>.json
```

### 2.4 이슈에 코멘트 추가

```bash
curl -s -X PUT -H "Content-Type: application/json" \
  -H "X-Redmine-API-Key: <API_KEY>" \
  -d '{"issue":{"notes":"코멘트 내용"}}' \
  http://211.117.60.5:8080/issues/<ISSUE_ID>.json
```

### 2.5 이슈 조회

```bash
# 단건
curl -s -H "X-Redmine-API-Key: <API_KEY>" \
  http://211.117.60.5:8080/issues/<ISSUE_ID>.json

# 프로젝트 열린 이슈 목록
curl -s -H "X-Redmine-API-Key: <API_KEY>" \
  "http://211.117.60.5:8080/issues.json?project_id=<PROJECT_ID>&status_id=open"
```

---

## 3. 레드마인 트래커 / 상태 ID

### 3.1 트래커 (tracker_id)

| ID | 트래커 | 용도 |
|:---:|------|------|
| 1 | 결함 | 버그 리포트 |
| 2 | 새기능 | 요구사항·기능 개발 |
| 3 | 지원 | 기타 요청 |

> ⚠️ **레드마인 기본값은 결함(1)**. 새기능 이슈 생성 시 반드시 `tracker_id: 2` 명시.

### 3.2 상태 (status_id)

| ID | 상태 | 구분 | 사용 시점 |
|:---:|------|:---:|------|
| 1 | 신규 | Open | 이슈 등록 직후 |
| 2 | 진행 | Open | 작업 착수 시 |
| 3 | 해결 | Open | 작업자 완료 보고 |
| 4 | 의견 | Open | 추가 논의 필요 |
| **5** | **완료** | **Closed** | **PM 최종 검토 후 종결** |
| **6** | **거절** | **Closed** | **반려·취소** |

### 3.3 2단계 종결 워크플로우 (필수)

```
작업자: status_id=3 (해결) ← 작업 완료 후 보고
         ↓
PM:     검토·검증
         ↓
PM:     status_id=5 (완료, Closed) ← 최종 종결
```

**진짜 종료는 5 또는 6만 Closed로 인정.** 3(해결)은 Open 상태이므로 종료 아님.

---

## 4. 프로젝트별 인스턴스에서 관리하는 값 (각 역할 persona.md)

| 값 | 위치 |
|----|------|
| project_id | `<프로젝트>/brain/PM/persona.md` 인스턴스 |
| 내 role user_id | `<프로젝트>/brain/<역할>/persona.md` 인스턴스 |
| 내 API 키 | `<프로젝트>/brain/<역할>/persona.md` 인스턴스 (git 비공유) |

---

## 5. cmux 팀 지시 프로토콜 (공통)

### 5.1 기본 명령어

```bash
cmux send --surface surface:XX "메시지"           # 텍스트 입력창에 작성
cmux send-key --surface surface:XX Enter          # 제출 (필수!)
cmux read-screen --surface surface:XX --lines 30  # 화면 확인
cmux browser --surface surface:XX <명령>          # 브라우저 제어 (필요 시)
```

> 각 에이전트에게 실제 배정된 surface 번호·별칭은 agiteam.sh 부팅 시 PM 시스템 프롬프트에 동적으로 주입된다.

### 5.2 필수 규칙

- **`cmux send` 후 반드시 `cmux send-key --surface <surface> Enter`** 로 제출
  (없으면 입력창에만 남고 실행 안됨 — 모든 에이전트 공통)
- **모든 에이전트 공통 적용**: Claude Code·Codex·Gemini CLI 전부 동일
- **한국어로 소통** — 유저·팀원 모든 대화는 한국어

### 5.3 에이전트별 특이점

| 에이전트 | 주의 |
|---------|------|
| Gemini CLI | 메시지가 길거나 한글·특수문자 많을 때 `cat > /tmp/msg.txt <<'EOF' ... EOF` 로 먼저 쓰고 `cmux send "$(cat /tmp/msg.txt)"` 방식 권장. 직접 send 시 한글 깨지거나 셸 모드 유발 가능 |
| Gemini 셸 모드 복구 | `cmux send-key --surface <surface> Escape` 로 탈출 후 재지시 |
| Codex (샌드박스) | localhost 접근이 제한될 수 있음. 네트워크 작업은 PM이 직접 실행 후 결과서만 Codex에 요청 |
| 공통 | 한 번의 `cmux send` 에 여러 파일·여러 작업 동시 요청 금지 — function call 에러 가능 |

### 5.4 팀 소통 원칙

- **PM 지시 없이 자발 착수 금지** — 각 역할은 세션 부팅 직후 `READY: <역할>` 출력 후 대기 (상세는 각 역할 persona의 "작업 착수 원칙" 섹션)
- **역할 간 직접 연락 금지** — 에이전트는 다른 에이전트와 어떤 형태로도 직접 소통하지 않는다. PM이 각 역할에게 독립적으로 지시한다. (원형 PM persona §3.13 통신 규율 준수)
- **완료 보고 시점**: 본인 R 산출물 작성 완료 → PM에게 보고 (다른 역할을 직접 호출하지 않음)

---

## 6. 산출물 버전 관리 정책 (SSOT)

> AgiTeamBuilder 전 산출물(문서·코드·시안)의 **버전관리 단일 진실 공급원(SSOT)**. 핵심은 **매 변경 시 단 두 동작**이다. 승격·게이트 개념은 없다.

### 6.1 정책 (매 변경 시 두 동작)

1. **백업**: 내용을 바꾸기 직전, 현재 파일을 같은 폴더 `_archive/<이름>_YYYYMMDDhhmmss.md`로 복사한다.
2. **갱신**: `<이름>.latest.md`에 최신 내용을 작성(덮어쓴다).

- 현행본 파일명은 항상 `<이름>.latest.md`로 고정. 참조·링크·startupFiles는 `.latest`만 가리킨다.
- 과거본은 타임스탬프(`YYYYMMDDhhmmss`)가 곧 순서·이력. `_archive/`는 순수 보존(비관리 대상).
- `_archive/`는 make.sh `rsync --exclude='_*/'`로 스캐폴딩 배제 + `.gitignore` `_archive/`로 git 추적 배제.

### 6.2 frontmatter 기록

- `last_updated`에 갱신 시점, 본문 **개정이력 섹션**에 변경 내용을 적는다.
- `version`(v0.1 → v0.2 → ...)은 **단순 증가 참고 라벨** — "몇 번째 개정"인지 표시일 뿐이다. 에이전트가 갱신할 때마다 자율로 올린다.
- **승격·메이저/마이너 구분·유저 승인 게이트는 없다.** version 숫자는 어떤 행위도 게이트하지 않으며, v1.0 같은 "정식 릴리즈 승격" 이벤트도 존재하지 않는다.

### 6.3 역할별 특화 (각 역할 persona에 잔존)

- **QA**: 변경 시 `_archive` 백업 누락·`latest` 파일명 위반 여부를 감리한다.
- **Designer**: 시안·퍼블리싱 파일(PNG·HTML)도 동일하게 두 동작을 적용한다.
- **각 역할**: 본인 R/A 산출물 변경 시 위 두 동작 + frontmatter 갱신 책임.

---

*본 문서는 AgiTeamBuilder 원형이다. 모든 프로젝트에 동일 적용되는 공용 상수만 포함. 역할별·프로젝트별 가변 값은 각 역할 `persona.md` 인스턴스에 기록.*
