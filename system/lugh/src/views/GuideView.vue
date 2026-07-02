<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'

const router = useRouter()

// 역할 카드 — 아코디언 (클릭 시 상세 펼치기)
const expandedRole = ref<string | null>(null)

function toggleRole(name: string) {
  expandedRole.value = expandedRole.value === name ? null : name
}

const roles = [
  {
    icon: '🧭', name: 'PM',
    desc: '프로젝트를 이끄는 의사결정자',
    detail: '수행계획서·WBS·산출물 리스트를 관리합니다. 레드마인 이슈 기반으로 팀원에게 작업을 지시하고, 진척률을 모니터링합니다. 모든 팀원의 유일한 시작점입니다.',
  },
  {
    icon: '🏛️', name: 'Architect',
    desc: '시스템 설계와 기술 방향 결정',
    detail: '아키텍처 설계서·API 명세서·ERD·화면 설계서를 작성합니다. 기술 스택 선택과 구성요소 간 연동 규격을 정의하고 PM의 승인을 받습니다.',
  },
  {
    icon: '⚙️', name: 'DeveloperBE',
    desc: '백엔드 로직과 API 구현',
    detail: '설계서 기반으로 서버·API·데이터베이스 로직을 구현합니다. 단위 테스트를 작성하고 QA 검증 전 빌드 성공을 확인합니다.',
  },
  {
    icon: '🖥️', name: 'DeveloperFE',
    desc: 'UI와 프론트엔드 구현',
    detail: 'Designer의 퍼블리싱 파일을 참조해 Vue/React 컴포넌트를 구현합니다. 백엔드 API와 IPC 연동을 담당하고 화면 동작을 검증합니다.',
  },
  {
    icon: '🚀', name: 'DevOps',
    desc: '빌드·배포·인프라 관리',
    detail: '개발 환경 구성, 빌드 파이프라인, 배포 자동화를 담당합니다. 환경 변수·시크릿 관리와 운영 서버 모니터링도 수행합니다.',
  },
  {
    icon: '🎨', name: 'Designer',
    desc: 'UX 설계와 비주얼 시안',
    detail: '화면 흐름과 UI 시안 이미지를 작성하고, HTML+CSS 퍼블리싱까지 담당합니다. 프레임워크 코드는 DeveloperFE가 담당합니다.',
  },
  {
    icon: '🔍', name: 'QA',
    desc: '품질 감리와 결함 관리',
    detail: '전체 기능 회귀 테스트, 설계-코드 정합성 감리, 결함 등록을 수행합니다. 결함은 품질관리대장에 기록하고 PM에게 보고합니다.',
  },
]

// 단계별 상세 — 탭 형태
const activeStep = ref(0)

const steps = [
  {
    num: '01', title: '프로젝트 설정',
    summary: '작업 폴더 경로와 AI API 키를 설정합니다',
    details: [
      {
        heading: '작업 폴더 설정',
        body: 'AgiTeamBuilder 프로젝트가 있는 폴더를 지정합니다. 폴더 안에는 agiteam.json 설정 파일이 있어야 합니다. 신규 프로젝트라면 설정 화면에서 새 프로젝트로 초기화합니다.',
      },
      {
        heading: 'AI API 키 입력',
        body: 'Claude(Anthropic), Codex(OpenAI), Gemini(Google) 중 사용할 에이전트의 API 키를 입력합니다. 키는 macOS Keychain에 암호화 저장되며 파일에는 남지 않습니다.',
      },
      {
        heading: '팀원 구성 확인',
        body: 'agiteam.json에서 7역할(PM·Architect·DeveloperBE·DeveloperFE·DevOps·Designer·QA)에 각각 어떤 AI 에이전트를 배정할지 확인합니다. 역할별로 Claude·Codex·Gemini 중 선택 가능합니다.',
      },
    ],
  },
  {
    num: '02', title: '팀 부팅',
    summary: 'AI 에이전트 7명을 동시에 부팅합니다',
    details: [
      {
        heading: '부팅 시작',
        body: '설정이 완료되면 Boot 화면으로 이동합니다. 팀 부팅 시작 버튼을 누르면 7개 에이전트가 각자의 터미널 세션에서 동시에 실행됩니다.',
      },
      {
        heading: 'READY 신호 확인',
        body: '각 팀원이 준비되면 READY 신호를 출력합니다. 화면에 7/7 READY가 표시되면 부팅 완료입니다. PM이 가장 먼저 부팅되고 나머지 6역할이 PM 지시를 기다립니다.',
      },
      {
        heading: '에이전트 배치 확인',
        body: '부팅 후 각 에이전트는 자신의 페르소나 파일을 읽고 이전 세션 맥락을 복원합니다. 세션이 끊겼다가 재연결해도 이전 작업 내용이 유지됩니다.',
      },
    ],
  },
  {
    num: '03', title: '작업 시작',
    summary: '산출물·레드마인·브라우저 패널로 팀과 협업합니다',
    details: [
      {
        heading: '산출물 패널',
        body: '프로젝트의 모든 산출물(문서·코드 인덱스)을 조회하고 편집합니다. 파일 수정 시 자동으로 _archive 폴더에 이전 버전이 백업되며 .latest.md 파일이 최신 상태를 유지합니다.',
      },
      {
        heading: '레드마인 패널',
        body: '운영 모드에서는 레드마인 이슈로 작업을 관리합니다. 이슈 목록 조회, 신규 이슈 등록, 상태 변경(신규→진행→해결→완료)을 앱 안에서 바로 처리합니다.',
      },
      {
        heading: '브라우저 패널',
        body: 'PM이 팀원에게 지시하거나 WebSearch로 자료를 찾을 때 내장 브라우저를 활용합니다. 레드마인 웹 화면, API 문서, 참고 자료를 별도 창 없이 바로 확인합니다.',
      },
      {
        heading: 'PM 지시 흐름',
        body: 'PM은 cmux를 통해 각 팀원 세션에 작업을 지시합니다. 팀원은 완료 후 PM에게 보고하고, PM이 다음 단계를 연결합니다. 모든 지시는 산출물 단위로 이루어집니다.',
      },
    ],
  },
]
</script>

<template>
  <div class="guide-root">
    <div class="guide-content">

      <!-- 헤더 -->
      <header class="guide-header">
        <h1 class="guide-title">Lugh</h1>
        <p class="guide-sub">모든 기술을 하나로, 7개의 AI가 하나의 팀으로</p>
      </header>

      <!-- 팀 소개 (아코디언) -->
      <section class="guide-section">
        <h2 class="section-heading">팀 소개</h2>
        <p class="section-desc">각 역할 카드를 클릭하면 상세 역할을 확인할 수 있습니다.</p>
        <div class="role-grid">
          <div
            v-for="r in roles" :key="r.name"
            class="role-card"
            :class="{ expanded: expandedRole === r.name }"
            @click="toggleRole(r.name)"
          >
            <div class="role-card-top">
              <span class="role-icon">{{ r.icon }}</span>
              <div class="role-info">
                <span class="role-name">{{ r.name }}</span>
                <span class="role-desc">{{ r.desc }}</span>
              </div>
              <span class="role-chevron">{{ expandedRole === r.name ? '▲' : '▼' }}</span>
            </div>
            <div v-if="expandedRole === r.name" class="role-detail">
              {{ r.detail }}
            </div>
          </div>
        </div>
      </section>

      <!-- 빠른 시작 (탭 형태) -->
      <section class="guide-section">
        <h2 class="section-heading">빠른 시작</h2>
        <!-- 탭 -->
        <div class="step-tabs">
          <button
            v-for="(s, i) in steps" :key="s.num"
            class="step-tab" :class="{ active: activeStep === i }"
            @click="activeStep = i"
          >
            <span class="tab-num">{{ s.num }}</span>
            <span class="tab-title">{{ s.title }}</span>
          </button>
        </div>
        <!-- 탭 내용 -->
        <div class="step-panel">
          <p class="step-summary">{{ steps[activeStep].summary }}</p>
          <div class="step-details">
            <div
              v-for="(d, i) in steps[activeStep].details" :key="i"
              class="step-detail-item"
            >
              <div class="detail-heading">
                <span class="detail-dot" />
                {{ d.heading }}
              </div>
              <p class="detail-body">{{ d.body }}</p>
            </div>
          </div>
        </div>
      </section>

      <!-- CTA -->
      <div class="guide-cta">
        <button class="btn-primary" @click="router.push('/settings')">프로젝트 설정 시작</button>
        <button class="btn-secondary" @click="router.push('/launcher')">홈으로</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.guide-root {
  width: 100%; height: 100%; overflow-y: auto;
  background: var(--bg-base); display: flex; justify-content: center;
}
.guide-content {
  width: min(800px, 100%); padding: 48px 24px;
  display: flex; flex-direction: column; gap: 48px;
}
/* 헤더 */
.guide-header { text-align: center; }
.guide-title {
  font-size: 52px; font-weight: 800; letter-spacing: -2px;
  background: linear-gradient(135deg, var(--accent), var(--primary));
  -webkit-background-clip: text; -webkit-text-fill-color: transparent;
}
.guide-sub { color: var(--text-muted); font-size: 16px; margin-top: 10px; }
/* 섹션 */
.section-heading { font-size: 18px; font-weight: 700; color: var(--text-primary); margin-bottom: 8px; }
.section-desc { font-size: 13px; color: var(--text-muted); margin-bottom: 16px; }
/* 역할 카드 아코디언 */
.role-grid { display: flex; flex-direction: column; gap: 8px; }
.role-card {
  background: var(--bg-panel); border: 1px solid var(--line);
  border-radius: 10px; padding: 14px 16px; cursor: pointer;
  transition: border-color 0.2s;
}
.role-card:hover, .role-card.expanded { border-color: var(--accent); }
.role-card-top { display: flex; align-items: center; gap: 12px; }
.role-icon { font-size: 22px; flex-shrink: 0; }
.role-info { flex: 1; display: flex; flex-direction: column; gap: 2px; }
.role-name { font-size: 14px; font-weight: 700; color: var(--text-primary); }
.role-desc { font-size: 12px; color: var(--text-muted); }
.role-chevron { font-size: 11px; color: var(--text-muted); }
.role-detail {
  margin-top: 12px; padding-top: 12px; border-top: 1px solid var(--line-soft);
  font-size: 13px; color: var(--text-soft); line-height: 1.7;
}
/* 단계 탭 */
.step-tabs { display: flex; gap: 8px; margin-bottom: 16px; flex-wrap: wrap; }
.step-tab {
  display: flex; align-items: center; gap: 8px; padding: 10px 20px;
  background: var(--bg-panel); border: 1px solid var(--line);
  border-radius: 8px; cursor: pointer; color: var(--text-muted);
  transition: all 0.2s;
}
.step-tab.active {
  border-color: var(--accent); background: rgba(99,102,241,0.1);
  color: var(--text-primary);
}
.tab-num { font-size: 18px; font-weight: 800; color: var(--accent); }
.tab-title { font-size: 13px; font-weight: 600; }
/* 탭 패널 */
.step-panel {
  background: var(--bg-panel); border: 1px solid var(--line);
  border-radius: 10px; padding: 24px;
}
.step-summary {
  font-size: 14px; font-weight: 600; color: var(--text-primary);
  margin-bottom: 20px; padding-bottom: 16px; border-bottom: 1px solid var(--line-soft);
}
.step-details { display: flex; flex-direction: column; gap: 20px; }
.step-detail-item {}
.detail-heading {
  display: flex; align-items: center; gap: 8px;
  font-size: 13px; font-weight: 700; color: var(--text-primary); margin-bottom: 6px;
}
.detail-dot {
  width: 6px; height: 6px; border-radius: 50%;
  background: var(--accent); flex-shrink: 0;
}
.detail-body { font-size: 13px; color: var(--text-muted); line-height: 1.7; padding-left: 14px; }
/* CTA */
.guide-cta { display: flex; justify-content: center; gap: 12px; padding-bottom: 24px; }
.btn-primary {
  padding: 12px 40px; background: var(--accent); color: #fff;
  border: none; border-radius: 8px; font-size: 14px; font-weight: 600;
  cursor: pointer; transition: opacity 0.2s;
}
.btn-primary:hover { opacity: 0.85; }
.btn-secondary {
  padding: 12px 40px; background: transparent; color: var(--text-muted);
  border: 1px solid var(--line); border-radius: 8px; font-size: 14px;
  cursor: pointer; transition: border-color 0.2s;
}
.btn-secondary:hover { border-color: var(--accent); color: var(--text-primary); }
</style>
