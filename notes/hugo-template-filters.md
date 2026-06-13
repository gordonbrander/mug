# Hugo template functions

Complete set of template functions Hugo exposes, organized by namespace (current as
of Hugo's docs, June 2026). Hugo groups everything into ~30 namespaces. Many of these
are also available under shorter legacy aliases (e.g. `where`, `partial`, `printf`,
`markdownify`), but the canonical namespaced names are below.

Source: [gohugo.io/functions](https://gohugo.io/functions/) and its per-namespace pages.

## cast
`cast.ToFloat` · `cast.ToInt` · `cast.ToString`

## collections
`collections.After` · `collections.Append` · `collections.Apply` · `collections.Complement` · `collections.Delimit` · `collections.Dictionary` · `collections.First` · `collections.Group` · `collections.In` · `collections.Index` · `collections.Intersect` · `collections.IsSet` · `collections.KeyVals` · `collections.Last` · `collections.Merge` · `collections.NewScratch` · `collections.Querify` · `collections.Reverse` · `collections.Seq` · `collections.Shuffle` · `collections.Slice` · `collections.Sort` · `collections.SymDiff` · `collections.Union` · `collections.Uniq` · `collections.Where`

## compare
`compare.Conditional` · `compare.Default` · `compare.Eq` · `compare.Ge` · `compare.Gt` · `compare.Le` · `compare.Lt` · `compare.Ne`

## crypto
`crypto.HMAC` · `crypto.MD5` · `crypto.SHA1` · `crypto.SHA256`

## css
`css.Build` · `css.PostCSS` · `css.Quoted` · `css.Sass` · `css.TailwindCSS` · `css.Unquoted`

## debug
`debug.Dump` · `debug.Timer` · `debug.VisualizeSpaces`

## diagrams
`diagrams.Goat`

## encoding
`encoding.Base64Decode` · `encoding.Base64Encode` · `encoding.Jsonify`

## fmt
`fmt.Errorf` · `fmt.Erroridf` · `fmt.Print` · `fmt.Printf` · `fmt.Println` · `fmt.Warnf` · `fmt.Warnidf`

## global
`global/page` (the `page` global) · `global/site` (the `site` global)

## hash
`hash.FNV32a` · `hash.XxHash`

## hugo
`hugo.BuildDate` · `hugo.CommitHash` · `hugo.Data` · `hugo.Deps` · `hugo.Environment` · `hugo.Generator` · `hugo.GoVersion` · `hugo.IsDevelopment` · `hugo.IsExtended` · `hugo.IsMultihost` · `hugo.IsMultilingual` · `hugo.IsProduction` · `hugo.IsServer` · `hugo.Sites` · `hugo.Store` · `hugo.Version` · `hugo.WorkingDir`

## images
`images.AutoOrient` · `images.Brightness` · `images.ColorBalance` · `images.Colorize` · `images.Config` · `images.Contrast` · `images.Dither` · `images.Filter` · `images.Gamma` · `images.GaussianBlur` · `images.Grayscale` · `images.Hue` · `images.Invert` · `images.Mask` · `images.Opacity` · `images.Overlay` · `images.Padding` · `images.Pixelate` · `images.Process` · `images.QR` · `images.Saturation` · `images.Sepia` · `images.Sigmoid` · `images.Text` · `images.UnsharpMask`

## inflect
`inflect.Humanize` · `inflect.Pluralize` · `inflect.Singularize`

## js
`js.Babel` · `js.Batch` · `js.Build`

## lang
`lang.FormatAccounting` · `lang.FormatCurrency` · `lang.FormatNumber` · `lang.FormatNumberCustom` · `lang.FormatPercent` · `lang.Merge` · `lang.Translate`

## math
`math.Abs` · `math.Acos` · `math.Add` · `math.Asin` · `math.Atan` · `math.Atan2` · `math.Ceil` · `math.Cos` · `math.Counter` · `math.Div` · `math.Floor` · `math.Log` · `math.Max` · `math.MaxInt64` · `math.Min` · `math.Mod` · `math.ModBool` · `math.Mul` · `math.Pi` · `math.Pow` · `math.Product` · `math.Rand` · `math.Round` · `math.Sin` · `math.Sqrt` · `math.Sub` · `math.Sum` · `math.Tan` · `math.ToDegrees` · `math.ToRadians`

## openapi3
`openapi3.Unmarshal`

## os
`os.FileExists` · `os.Getenv` · `os.ReadDir` · `os.ReadFile` · `os.Stat`

## partials
`partials.Include` · `partials.IncludeCached`

## path
`path.Base` · `path.BaseName` · `path.Clean` · `path.Dir` · `path.Ext` · `path.Join` · `path.Split`

## reflect
`reflect.IsImageResource` · `reflect.IsImageResourceProcessable` · `reflect.IsImageResourceWithMeta` · `reflect.IsMap` · `reflect.IsPage` · `reflect.IsResource` · `reflect.IsSite` · `reflect.IsSlice`

## resources
`resources.ByType` · `resources.Concat` · `resources.Copy` · `resources.ExecuteAsTemplate` · `resources.Fingerprint` · `resources.FromString` · `resources.Get` · `resources.GetMatch` · `resources.GetRemote` · `resources.Match` · `resources.Minify` · `resources.PostProcess`

## safe
`safe.CSS` · `safe.HTML` · `safe.HTMLAttr` · `safe.JS` · `safe.JSStr` · `safe.URL`

## strings
`strings.Chomp` · `strings.Contains` · `strings.ContainsAny` · `strings.ContainsNonSpace` · `strings.Count` · `strings.CountRunes` · `strings.CountWords` · `strings.Diff` · `strings.FindRE` · `strings.FindRESubmatch` · `strings.FirstUpper` · `strings.HasPrefix` · `strings.HasSuffix` · `strings.Repeat` · `strings.Replace` · `strings.ReplacePairs` · `strings.ReplaceRE` · `strings.RuneCount` · `strings.SliceString` · `strings.Split` · `strings.Substr` · `strings.Title` · `strings.ToLower` · `strings.ToUpper` · `strings.Trim` · `strings.TrimLeft` · `strings.TrimPrefix` · `strings.TrimRight` · `strings.TrimSpace` · `strings.TrimSuffix` · `strings.Truncate`

## templates
`templates.Current` · `templates.Defer` · `templates.Exists` · `templates.Inner`

## time
`time.AsTime` · `time.Duration` · `time.Format` · `time.In` · `time.Now` · `time.ParseDuration`

## transform
`transform.CanHighlight` · `transform.Emojify` · `transform.Highlight` · `transform.HighlightCodeBlock` · `transform.HTMLEscape` · `transform.HTMLToMarkdown` · `transform.HTMLUnescape` · `transform.Markdownify` · `transform.Plainify` · `transform.PortableText` · `transform.Remarshal` · `transform.ToMath` · `transform.Unmarshal` · `transform.XMLEscape`

## urls
`urls.AbsLangURL` · `urls.AbsURL` · `urls.Anchorize` · `urls.JoinPath` · `urls.Parse` · `urls.PathEscape` · `urls.PathUnescape` · `urls.Ref` · `urls.RelLangURL` · `urls.RelRef` · `urls.RelURL` · `urls.URLize`

## Go text/template built-ins (inherited)
Control/keywords and native funcs Hugo inherits from Go: `and` · `block` · `break` · `continue` · `define` · `else` · `end` · `if` · `len` · `not` · `or` · `range` · `return` · `template` · `try` · `urlquery` · `with` (plus the standard Go funcs like `index`, `print`/`printf`/`println`, `html`, `js`, `call`, `eq`/`ne`/`lt`/`le`/`gt`/`ge` which Hugo also surfaces via its own namespaces).

## Notes

- The **`global`** entries (`page`, `site`) aren't functions but the two top-level context objects every template gets.
- Several namespaces are **build-pipeline** features rather than pure string helpers (`css.*`, `js.*`, `resources.*`, `images.*`, `transform.Highlight`) — they operate on Hugo's resource/asset pipeline, which has no direct Tera analog.
- Hugo keeps **unnamespaced aliases** for most of these for backward compatibility (`where`, `first`, `partial`, `partialCached`, `markdownify`, `printf`, `safeHTML`, `urlize`, `dict`, `slice`, etc.), so older themes use the short names while docs use the namespaced ones.
