{{/*
Expand a container image reference from a small values map.

The map may carry:
  image.full     - a complete reference (used verbatim)
  image.registry - registry host, default ghcr.io/daniel-white/resume-analyzer
  image.repository - repository path (required when full is absent)
  image.tag      - tag or digest (required when full is absent)

Local values keep using the plain `image: resume-analyzer/api:dev` string, so
this helper is additive: release values supply registry/repository/tag (or a
digest) and the templates read `.Values.api.image` / `.Values.web.image` as
before. The helper exists so release packaging and future templates build
references in one place instead of string-concatenating in each template.
*/}}
{{- define "resume-analyzer.image" -}}
{{- $img := .image -}}
{{- if and $img (hasKey $img "full") -}}
{{- $img.full -}}
{{- else -}}
{{- $registry := (index $img "registry" | default "ghcr.io/daniel-white/resume-analyzer") -}}
{{- $repository := (index $img "repository" | required "resume-analyzer.image: repository is required") -}}
{{- $tag := (index $img "tag" | required "resume-analyzer.image: tag is required") -}}
{{- printf "%s/%s:%s" $registry $repository $tag -}}
{{- end -}}
{{- end -}}

{{/*
Default pull policy: Never for local :dev images, IfNotPresent for release
images pinned to an immutable tag/digest. Call sites pass .Values.api or
.Values.web and read .pullPolicy.
*/}}
{{- define "resume-analyzer.pullPolicy" -}}
{{- $p := .pullPolicy -}}
{{- if $p -}}
{{- $p -}}
{{- else -}}
{{- if (hasSuffix ":dev" (default "" .image)) -}}
Never
{{- else -}}
IfNotPresent
{{- end -}}
{{- end -}}
{{- end -}}
