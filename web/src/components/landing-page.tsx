import { Link } from "@tanstack/react-router";
import { ArrowDown, ArrowRight, ArrowUpRight, BookOpen, Check, CheckCheck, FileSearch, FileText, LoaderCircle, Mail, ScanLine, ShieldCheck, Sparkles, Upload, X } from "lucide-react";
import { useEffect, useRef, useState, type DragEvent, type FormEvent } from "react";

import { detectTimeZone } from "~/lib/timezone";
import { sendMagicLink } from "~/fns/auth.functions";
import { stageUpload } from "~/fns/resumes.functions";

export function LandingPage() {
  const [upload, setUpload] = useState<{ id: string; filename: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [email, setEmail] = useState("");
  const [sent, setSent] = useState(false);
  const depth = useRef(0);
  const input = useRef<HTMLInputElement>(null);
  const [timezone, setTimezone] = useState<string | null>(null);

  useEffect(() => {
    setTimezone(detectTimeZone());
  }, []);

  async function store(file?: File) {
    if (!file || busy) return;
    if (!/\.(pdf|docx)$/i.test(file.name)) { setError("Choose a PDF or DOCX résumé."); return; }
    if (file.size > 5 * 1024 * 1024) { setError("That file is over five megabytes."); return; }
    setBusy(true);
    setError(null);
    const body = new FormData();
    body.set("file", file, file.name);
    try {
      const result = await stageUpload({ data: body });
      if (!result.ok) { setError(result.message); return; }
      setUpload({ id: result.id, filename: file.name });
      setSent(false);
    } catch { setError("Your résumé did not upload. Please try again."); }
    finally { setBusy(false); }
  }

  function drop(event: DragEvent<HTMLDivElement>) {
    event.preventDefault();
    depth.current = 0;
    setDragging(false);
    if (event.dataTransfer.files.length > 1) { setError("Please upload one résumé at a time."); return; }
    void store(event.dataTransfer.files[0]);
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!upload || busy) return;
    setBusy(true);
    setError(null);
    try {
      const result = await sendMagicLink({ data: { email, uploadId: upload.id, returnTo: `/resumes/${upload.id}` } });
      if (!result.ok) { setError(result.message); return; }
      setSent(true);
    } catch { setError("The sign-in email did not send. Please try again."); }
    finally { setBusy(false); }
  }

  return (
    <div className="landing">
      <header className="landing-nav">
        <Link to="/" className="landing-brand"><span><BookOpen size={23} aria-hidden="true" /></span>The Other Read<span className="landing-beta">BETA</span></Link>
        <nav aria-label="Main navigation"><a href="#how-it-works">How it works</a><Link to="/login" className="landing-signin">Sign in <ArrowUpRight size={16} aria-hidden="true" /></Link></nav>
      </header>

      <main>
        <section className="landing-hero" aria-labelledby="hero-title">
          <div className="hero-copy">
            <p className="landing-eyebrow"><span /> A different perspective. A stronger résumé.</p>
            <h1 id="hero-title">You see a résumé.<br />They see <span className="hero-highlight">data.<svg viewBox="0 0 330 20" preserveAspectRatio="none" aria-hidden="true"><path d="M3 14 Q150 -5 325 9 M20 19 Q180 3 310 16" /></svg></span></h1>
            <p className="hero-description">Before you hit apply, see what hiring software actually picks up. Find the missing details. Fix the blind spots. Make your experience count.</p>
            <a className="hero-jump" href="#upload">Give your résumé another read <ArrowDown size={18} aria-hidden="true" /></a>
            <div className="hero-promises"><span><Check size={16} /> Upload before signing in</span><span><Check size={16} /> No password needed</span></div>
          </div>
          <div className="hero-preview" aria-label="Illustrative résumé analysis, not a real result">
            <div className="preview-orbit" aria-hidden="true" />
            <div className="preview-document">
              <div className="preview-document-top"><span><FileText size={15} /> your-next-chapter.pdf</span><span>01</span></div>
              <div className="preview-document-body"><span className="preview-kicker">EXPERIENCE, MEET OPPORTUNITY</span><h2>Alex Morgan</h2><p>Product designer · New York, NY</p><div className="preview-rule" /><h3>Experience</h3><strong>Senior Product Designer</strong><p>Turning complex problems into thoughtful products.</p><div className="preview-lines"><i /><i /><i /></div><h3>Skills</h3><div className="preview-skills"><span>Design systems</span><span>Research</span><span>Prototyping</span></div></div>
              <div className="preview-scan"><ScanLine size={16} /> Looking beyond the layout</div>
            </div>
            <div className="preview-score"><span className="preview-score-icon"><Sparkles size={18} /></span><div><span>The other read</span><strong>Small fixes.<br />Big difference.</strong></div><ArrowUpRight size={23} /></div>
            <div className="preview-finding"><span className="finding-icon"><FileSearch size={23} /></span><div><strong>Great skills. Wrong place.</strong><p>A table can hide your strengths.<br />Try moving skills into plain text.</p></div><span className="finding-tag">LAYOUT CHECK</span></div>
            <span className="preview-caption">ILLUSTRATIVE REPORT · YOUR RESULTS WILL VARY</span>
            <span className="preview-star" aria-hidden="true">✳</span>
          </div>
        </section>

        <section className="landing-upload-section" id="upload" aria-labelledby="upload-title">
          <div className="upload-intro"><span className="landing-eyebrow">YOUR NEXT MOVE STARTS HERE</span><h2 id="upload-title">Less guessing.<br />More <em>getting noticed.</em></h2><p>Start with the résumé you already have.<br />We’ll help you see it in a whole new way.</p><div className="upload-privacy"><ShieldCheck size={20} /><span>Private by design.<br /><small>Temporary uploads expire after one hour.</small></span></div></div>
          <div className={`landing-upload ${dragging ? "is-dragging" : ""}`} aria-busy={busy}
            onDragEnter={(event) => { event.preventDefault(); depth.current++; if (!busy && !upload) setDragging(true); }}
            onDragOver={(event) => { event.preventDefault(); event.dataTransfer.dropEffect = busy || upload ? "none" : "copy"; }}
            onDragLeave={(event) => { event.preventDefault(); if (--depth.current <= 0) setDragging(false); }}
            onDrop={(event) => { if (upload) { event.preventDefault(); return; } drop(event); }}>
            {!upload ? <>
              <span className="landing-upload-icon">{busy ? <LoaderCircle className="busy-icon" size={30} /> : <Upload size={30} strokeWidth={1.6} />}</span>
              <h3>{busy ? "Giving your résumé a safe landing…" : dragging ? "Drop it. Let’s take a look." : "Drop your résumé. Raise your game."}</h3>
              <p>Drag your file here, or pick it from your device.</p>
              <input ref={input} className="sr-only" type="file" aria-label="Upload résumé" accept=".pdf,.docx,application/pdf,application/vnd.openxmlformats-officedocument.wordprocessingml.document" disabled={busy} onChange={(event) => { void store(event.target.files?.[0]); event.target.value = ""; }} />
              <button type="button" className="landing-primary" disabled={busy} onClick={() => input.current?.click()}>{busy ? "Uploading…" : "Choose my résumé"}<ArrowUpRight size={19} /></button>
              <span className="landing-formats">PDF or DOCX · Up to 5 MB</span><span className="landing-no-account">No account needed to upload. Sign in to start your analysis.</span>
            </> : sent ? <div className="upload-next" role="status"><span className="landing-upload-icon"><Mail size={29} /></span><h3>Your next chapter is in your inbox.</h3><p>We sent a sign-in link to <strong>{email}</strong>. Open it within 15 minutes to start your analysis. Your résumé comes with you.</p><button className="landing-secondary" onClick={() => setSent(false)}>Use a different email <ArrowRight size={16} /></button></div> : <form className="upload-next" onSubmit={submit}>
              <div className="uploaded-file"><FileText size={21} /><span>{upload.filename}<small><CheckCheck size={13} /> Uploaded. No need to choose it again.</small></span><button type="button" aria-label="Choose a different résumé" disabled={busy} onClick={() => { setUpload(null); setError(null); }}><X size={18} /></button></div>
              <h3>One link. Then the full picture.</h3><p>Sign in to start your analysis and keep your results together. No password, no re-upload.</p>
              <label htmlFor="landing-email">Your email</label><input id="landing-email" type="email" autoComplete="email" required value={email} onChange={(event) => setEmail(event.target.value)} placeholder="you@example.com" disabled={busy} />
              <button className="landing-primary" type="submit" disabled={busy}>{busy ? "Sending your link…" : "Email me a link"}<ArrowRight size={18} /></button>
            </form>}
            {error ? <p className="landing-error" role="alert">{error}</p> : null}
            {busy ? <span className="sr-only" role="status">{upload ? "Sending sign-in link." : "Uploading your résumé."}</span> : null}
          </div>
        </section>

        <section className="landing-how" id="how-it-works" aria-labelledby="how-title">
          <div className="how-heading"><p className="landing-eyebrow">NOT ANOTHER BLACK-BOX SCORE</p><h2 id="how-title">The same résumé.<br /><span>A different read.</span></h2><p>Hiring systems don’t all read the same way. Compare simulated ATS readers to understand what survives—and what gets lost.</p></div>
          <div className="landing-steps">
            <article><span className="step-number">01 / UPLOAD</span><Upload size={27} /><h3>Bring your experience.</h3><p>Drop a PDF or DOCX. Then sign in with an email link to start the analysis.</p></article>
            <article><span className="step-number">02 / UNDERSTAND</span><ScanLine size={27} /><h3>See the other side.</h3><p>Compare scores, extracted details, and parsing blind spots across different readers.</p></article>
            <article><span className="step-number">03 / IMPROVE</span><Sparkles size={27} /><h3>Make your next version count.</h3><p>Get practical fixes, upload a new version, and compare what changed.</p></article>
          </div>
        </section>
        <section className="landing-bottom"><span aria-hidden="true">✳</span><h2>Your experience deserves<br />to come through.</h2><a href="#upload" className="landing-primary">Let’s give it another read <ArrowUpRight size={20} /></a></section>
      </main>
      <footer className="landing-footer"><Link to="/" className="landing-brand"><BookOpen size={19} /> The Other Read</Link><p>Simulated ATS readers. Real perspective.<br />Not affiliated with hiring platforms. No interview guarantees.</p><a href="#upload">Back to your next move <ArrowUpRight size={15} /></a></footer>
    </div>
  );
}
