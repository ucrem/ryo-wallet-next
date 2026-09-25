import { useState } from "react"
import { openUrl } from "@tauri-apps/plugin-opener"
import { bundledChangelog } from "@/lib/bundledChangelog"
import { releaseUrl } from "@/lib/appVersion"
import type { AppVersionInfo } from "@/lib/useAppVersion"

export function About({ appVersion }: { appVersion: AppVersionInfo }) {
  const [openError, setOpenError] = useState(false)
  const url = appVersion.version ? releaseUrl(appVersion.version) : null

  async function openRelease() {
    if (!url) return
    setOpenError(false)
    try {
      await openUrl(url)
    } catch {
      setOpenError(true)
    }
  }

  return (
    <div className="min-w-0 pb-8">
      <p className="text-xs font-semibold tracking-[0.16em] text-sky-300">PROJECT</p>
      <h1 className="mt-3 text-3xl font-semibold tracking-tight">About</h1>
      <p className="mt-3 text-sm leading-6 text-slate-400">
        An independent open-source desktop wallet project for Ryo.
      </p>

      <section aria-labelledby="about-app" className="mt-7 rounded-xl border border-slate-700 bg-[#151d27] p-5 sm:p-6">
        <h2 id="about-app" className="text-xl font-semibold">Ryo Wallet Next</h2>
        <p className="mt-2 text-sm leading-6 text-slate-300">
          This is a development preview, not an official Ryo Currency wallet. Wallet operations are still in development.
        </p>
        <div className="mt-5 flex flex-wrap items-center gap-3 border-t border-slate-700 pt-5">
          <div>
            <p className="text-xs font-medium uppercase tracking-wide text-slate-400">Version</p>
            <p className="mt-1 font-mono text-lg text-slate-100" role="status">
              {appVersion.label ?? (appVersion.isLoading ? "Checking…" : "Unavailable")}
            </p>
          </div>
          {appVersion.status ? (
            <span className="rounded-full border border-sky-500/50 bg-sky-400/10 px-3 py-1 text-xs font-medium text-sky-200">
              {appVersion.status}
            </span>
          ) : null}
        </div>
        {!appVersion.isNative ? (
          <p className="mt-2 text-xs text-slate-400">Browser preview: version shown from the bundled changelog, not a native executable.</p>
        ) : appVersion.isError ? (
          <p className="mt-2 text-xs text-red-300" role="alert">The installed application version could not be read.</p>
        ) : null}
        {url && appVersion.isNative ? (
          <button type="button" onClick={() => void openRelease()}
            aria-label={`View ${appVersion.label} release on GitHub in your browser`}
            className="mt-5 rounded-lg border border-sky-500/60 px-4 py-2 text-sm font-medium text-sky-200 hover:bg-sky-400/10 focus-visible:outline-2 focus-visible:outline-sky-400">
            View this release on GitHub ↗
          </button>
        ) : url ? (
          <a href={url} target="_blank" rel="noopener noreferrer"
            aria-label={`View ${appVersion.label} release on GitHub in a new tab`}
            className="mt-5 inline-block rounded-lg border border-sky-500/60 px-4 py-2 text-sm font-medium text-sky-200 hover:bg-sky-400/10 focus-visible:outline-2 focus-visible:outline-sky-400">
            View this release on GitHub ↗
          </a>
        ) : null}
        {openError ? <p className="mt-2 text-sm text-red-300" role="alert">Could not open your browser. Please try again.</p> : null}
      </section>

      <section aria-labelledby="changelog-title" className="mt-9">
        <h2 id="changelog-title" className="text-xl font-semibold">What&apos;s new</h2>
        <p className="mt-1 text-sm text-slate-400">Release notes included with this application, available offline.</p>
        <div className="mt-5 grid gap-4">
          {bundledChangelog.map((release) => {
            const current = appVersion.isNative && release.version === appVersion.version
            return (
              <article key={release.version} className={"min-w-0 rounded-xl border bg-[#151d27] p-5 sm:p-6 " +
                (current ? "border-sky-500/70" : "border-slate-700")}>
                <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
                  <h3 className="font-mono text-base font-semibold text-slate-100">v{release.version}</h3>
                  {current ? <span className="text-xs font-medium text-sky-200">Installed version</span> : null}
                </div>
                <time dateTime={release.date} className="mt-1 block text-sm text-slate-400">{formatDate(release.date)}</time>
                {release.sections.map((section) => (
                  <div key={section.title} className="mt-5">
                    <h4 className="text-sm font-semibold text-slate-200">{section.title}</h4>
                    <ul className="mt-2 list-disc space-y-1.5 pl-5 text-sm leading-6 text-slate-300">
                      {section.items.map((item) => <li key={item}>{item}</li>)}
                    </ul>
                  </div>
                ))}
              </article>
            )
          })}
        </div>
      </section>
    </div>
  )
}

function formatDate(date: string): string {
  return new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "long", year: "numeric", timeZone: "UTC" })
    .format(new Date(`${date}T00:00:00Z`))
}
