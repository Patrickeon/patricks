// 채팅 마크다운 렌더러 (Redmine #30)
//
// AI 응답 메시지를 마크다운 → HTML로 변환한다. 코드블록·인라인코드·표·리스트·링크·
// 볼드/이탤릭을 지원한다. 보안(XSS 방지)은 다음 2중 방어로 처리한다.
//   1) 원시 HTML(블록/인라인 html 토큰)을 실행하지 않고 평문으로 이스케이프 → <script> 등 무력화
//   2) 링크/이미지 URL은 안전 프로토콜(http/https/mailto/tel)만 허용 → javascript: 등 차단
// 이 방식은 marked 옵션만으로 sanitize를 달성하므로 별도 의존성(DOMPurify 등)이 필요 없다.
//
// [격리] DeliverableView가 전역 `marked`를 쓰므로, 채팅용은 별도 Marked 인스턴스를 만들어
// 문서 뷰어 렌더링에 영향을 주지 않는다.
import { Marked, type Token, type Tokens } from 'marked'

/** 링크/이미지에 허용할 안전 프로토콜 */
const SAFE_URL = /^(https?:|mailto:|tel:)/i

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
}

const md = new Marked({ gfm: true, breaks: true })

md.use({
  // 안전하지 않은 URL은 렌더 전에 무력화한다.
  walkTokens(token: Token) {
    if (token.type === 'link' || token.type === 'image') {
      const t = token as Tokens.Link | Tokens.Image
      const href = (t.href ?? '').trim()
      if (!SAFE_URL.test(href)) t.href = ''
    }
  },
  renderer: {
    // 원시 HTML은 실행하지 않고 평문으로 표시 (XSS 차단). 블록 html·인라인 tag 토큰 모두 처리.
    html(token: Tokens.HTML | Tokens.Tag): string {
      return escapeHtml(token.text)
    },
  },
})

/**
 * 마크다운 문자열을 안전한 HTML로 렌더링한다.
 * - 스트리밍 중 부분 마크다운도 예외 없이 처리된다(marked는 불완전 입력을 관용).
 * - 반환 HTML의 모든 `<a>`에는 `data-external`·`rel`을 부여해, 클릭 시 컴포넌트가
 *   OS 기본 브라우저로 위임하도록 표식을 남긴다(웹뷰 내 네비게이션 방지).
 */
export function renderMarkdown(src: string): string {
  if (!src) return ''
  const html = md.parse(src, { async: false }) as string
  return html.replace(/<a /g, '<a data-external="1" rel="noopener noreferrer" ')
}
