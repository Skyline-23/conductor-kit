package main

import (
	"regexp"
	"sort"
	"strings"
)

var (
	pathExtPattern = regexp.MustCompile(`(?i)(^|\\s)([\\w./-]+\\.(go|mod|sum|md|markdown|json|yaml|yml|toml|ini|cfg|conf|ts|tsx|js|jsx|mjs|cjs|py|rb|java|kt|swift|c|cc|cpp|h|hpp|rs|lua|sh|zsh|bash|sql|csv|tsv|log|txt|xml|html|css|scss|sass|less))`)
)

func autoSelectRoles(prompt string, cfg Config) []string {
	available := map[string]bool{}
	for name := range cfg.Roles {
		available[name] = true
	}
	add := func(out []string, role string) []string {
		if !available[role] {
			return out
		}
		for _, existing := range out {
			if existing == role {
				return out
			}
		}
		return append(out, role)
	}

	text := strings.ToLower(prompt)
	out := []string{}

	// Always include the coordinator if configured.
	out = add(out, "oracle")

	if shouldUseLibrarian(prompt, text) {
		out = add(out, "librarian")
	}
	if containsAny(text, exploreKeywords()) {
		out = add(out, "explore")
	}
	if containsAny(text, uiKeywords()) {
		out = add(out, "frontend-ui-ux-engineer")
	}
	if containsAny(text, docKeywords()) {
		out = add(out, "document-writer")
	}
	if containsAny(text, multimodalKeywords()) {
		out = add(out, "multimodal-looker")
	}

	for name := range cfg.Roles {
		if name != "" && strings.Contains(text, strings.ToLower(name)) {
			out = add(out, name)
		}
	}

	if len(out) == 0 {
		roles := make([]string, 0, len(cfg.Roles))
		for name := range cfg.Roles {
			roles = append(roles, name)
		}
		sort.Strings(roles)
		out = append(out, roles...)
	}
	return out
}

func shouldUseLibrarian(prompt, lowered string) bool {
	if containsAny(lowered, librarianKeywords()) {
		return true
	}
	if pathExtPattern.MatchString(prompt) {
		return true
	}
	if strings.Contains(prompt, "/") || strings.Contains(prompt, `\`) {
		return true
	}
	return false
}

func containsAny(text string, keywords []string) bool {
	for _, k := range keywords {
		if k != "" && strings.Contains(text, k) {
			return true
		}
	}
	return false
}

func librarianKeywords() []string {
	return []string{
		"read file", "open file", "open the file", "read the file", "summarize file",
		"file", "files", "path", "paths", "repo", "repository", "codebase", "source",
		"파일", "읽", "열어", "경로", "레포", "리포", "코드베이스",
	}
}

func exploreKeywords() []string {
	return []string{
		"search", "research", "investigate", "compare", "options", "survey", "scan",
		"find", "discover", "alternative", "benchmark",
		"검색", "조사", "비교", "대안", "옵션", "찾아", "탐색",
	}
}

func uiKeywords() []string {
	return []string{
		"ui", "ux", "design", "frontend", "css", "tailwind", "react", "layout", "component",
		"디자인", "프론트", "프론트엔드", "레이아웃", "컴포넌트",
	}
}

func docKeywords() []string {
	return []string{
		"doc", "docs", "documentation", "readme", "guide", "manual", "changelog", "release notes",
		"문서", "가이드", "설명", "릴리즈 노트", "변경 로그",
	}
}

func multimodalKeywords() []string {
	return []string{
		"image", "images", "screenshot", "photo", "diagram", "mockup",
		"이미지", "사진", "스크린샷", "스샷", "다이어그램",
	}
}
