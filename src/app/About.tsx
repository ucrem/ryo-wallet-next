import { useState } from "react"
import { openUrl } from "@tauri-apps/plugin-opener"
import { bundledChangelog } from "@/lib/bundledChangelog"
import { releaseUrl } from "@/lib/appVersion"
import type { AppVersionInfo } from "@/lib/useAppVersion"
import type { AppUpdates } from "@/lib/useAppUpdates"
import { Button } from "@/components/ui/button"

export function About({ appVersion, updates }: { appVersion: AppVersionInfo; updates: AppUpdates }) {
  const [openError, setOpenError] = useState(false)
  const [backupConfirmation, setBackupConfirmation] = useState({ version: "", confirmed: false })
  const url = appVersion.version ? releaseUrl(appVersion.version) : null
  const availableUpdate = updates.result?.state === "available" ? updates.result : null
  const backupConfirmed = backupConfirmation.version === availableUpdate?.version && backupConfirmation.confirmed

  async function openRelease(target = url) {
    if (!target) return
    setOpenError(false)
    try {
      await openUrl(target)
    } catch {
      setOpenError(true)
    }
  }

  return (
    <div className="min-w-0 pb-8">
      <section aria-labelledby="about-app" className="rounded-xl border border-slate-700 bg-[#151d27] p-5 sm:p-6">
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
      </section>

      <section aria-labelledby="updates-title" className="mt-5 rounded-xl border border-slate-700 bg-[#151d27] p-5 sm:p-6">
        <h2 id="updates-title" className="text-xl font-semibold">App updates</h2>
        <p className="mt-2 text-sm text-slate-400">The installed app checks for new releases when it opens. You can also check here.</p>
        <Button
          type="button"
          variant="outline"
          className="mt-4"
          onClick={() => { setBackupConfirmation({ version: "", confirmed: false }); void updates.checkNow() }}
          disabled={
            !appVersion.isNative ||
            updates.checking ||
            updates.installing ||
            updates.result?.state === "development"
          }
        >
          {updates.checking
            ? "Checking…"
            : updates.result?.state === "development"
              ? "Unavailable in development"
              : "Check for updates"}
        </Button>
        {updates.checking ? (
          <div className="mt-4 flex items-center gap-3 rounded-lg border border-sky-500/40 bg-sky-400/10 p-4 text-sm text-sky-100" role="status" aria-live="polite">
            <span className="size-4 shrink-0 animate-spin rounded-full border-2 border-sky-300 border-t-transparent" aria-hidden="true" />
            Checking for updates…
          </div>
        ) : updates.result?.state === "development" ? (
          <p className="mt-3 text-sm text-slate-400" role="status">Update checks and installation are available in installed builds.</p>
        ) : updates.result?.state === "current" ? (
          <p className="mt-3 rounded-lg border border-slate-600 bg-slate-800/50 p-4 text-sm text-slate-200" role="status" aria-live="polite">
            Ryo Wallet Next is already up to date{appVersion.label ? ` (${appVersion.label})` : ""}.
          </p>
        ) : availableUpdate ? (
          <div className="mt-4 rounded-lg border border-sky-500/40 bg-sky-400/10 p-4" role="status">
            <p className="font-medium text-sky-100">Version v{availableUpdate.version} is available</p>
            {availableUpdate.notes ? <p className="mt-2 whitespace-pre-wrap text-sm text-slate-300">{availableUpdate.notes}</p> : null}
            <div className="mt-4 rounded-lg border border-amber-400/50 bg-amber-400/10 p-4 text-sm text-amber-100" role="alert">
              Before updating, back up your wallet files and recovery phrase or other wallet secrets in a safe place. The wallet will be locked before installation.
            </div>
            {availableUpdate.install_in_app ? (
              <>
                <label className="mt-4 flex items-start gap-3 text-sm text-slate-200">
                  <input type="checkbox" className="mt-0.5 size-4 accent-sky-400" checked={backupConfirmed}
                    onChange={(event) => setBackupConfirmation({ version: availableUpdate.version, confirmed: event.target.checked })} disabled={updates.installing} />
                  I have backed up my wallet files and recovery phrase.
                </label>
                <div className="mt-4 flex flex-wrap gap-3">
                  <Button type="button" className="bg-sky-400 text-slate-950 hover:bg-sky-300"
                    disabled={updates.installing || !backupConfirmed}
                    onClick={() => { setBackupConfirmation({ version: "", confirmed: false }); void updates.install(availableUpdate.version) }}>
                    {updates.installing ? "Downloading and installing…" : "Update"}
                  </Button>
                  <Button type="button" variant="outline"
                    disabled={updates.installing} onClick={() => void openRelease(releaseUrl(availableUpdate.version))}>
                    Open release page ↗
                  </Button>
                </div>
                <p className="mt-2 text-xs text-slate-400">Your system may ask for authorization to install the update.</p>
                {updates.installing && updates.progress ? (
                  <p className="mt-2 text-sm text-slate-300">
                    Downloaded {formatMegabytes(updates.progress.downloaded)}
                    {updates.progress.total ? ` of ${formatMegabytes(updates.progress.total)}` : ""}
                  </p>
                ) : null}
              </>
            ) : (
              <>
                <p className="mt-2 text-sm text-slate-300">For a DEB or RPM installation, download the matching new package and install it with your package manager.</p>
                <Button type="button" variant="outline" className="mt-4"
                  onClick={() => void openRelease(releaseUrl(availableUpdate.version))}>
                  Open release page ↗
                </Button>
              </>
            )}
          </div>
        ) : null}
        {updates.error ? <p className="mt-3 text-sm text-red-300" role="alert">{updates.error}</p> : null}
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

function formatMegabytes(bytes: number): string {
  return `${(bytes / 1_000_000).toFixed(1)} MB`
}

function formatDate(date: string): string {
  return new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "long", year: "numeric", timeZone: "UTC" })
    .format(new Date(`${date}T00:00:00Z`))
}
