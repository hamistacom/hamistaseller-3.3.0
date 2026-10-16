// The one running job (scan or campaign). Pages subscribe; the sidebar card always shows it.
import { invoke, newChannel, listen, toast } from "./core.js";

export const job = {
  kind: "", // scan | campaign
  running: false,
  title: "",
  status: "",
  progress: null, // {keyword, collected, checked, found, target}
  sending: null, // {sent, failed, skipped, total}
  captcha: "",
  logs: [],
  found: [],
  result: null,
};

const listeners = new Set();
let queued = false;
function notify() {
  if (queued) return;
  queued = true;
  requestAnimationFrame(() => {
    queued = false;
    listeners.forEach((fn) => fn(job));
  });
}

export function onJob(fn) {
  listeners.add(fn);
  fn(job);
  return () => listeners.delete(fn);
}

function onEvent(e) {
  switch (e.type) {
    case "status":
      job.status = e.message;
      job.captcha = "";
      break;
    case "progress":
      job.progress = e;
      break;
    case "found":
      job.found.unshift(e.place);
      if (job.found.length > 80) job.found.pop();
      break;
    case "log":
      job.logs.push(e);
      if (job.logs.length > 500) job.logs.shift();
      break;
    case "captcha":
      job.captcha = e.message;
      toast(e.message);
      break;
    case "sending":
      job.sending = e;
      break;
  }
  notify();
}

/** Jobs the scheduler starts in the background show up like the user's own (sidebar card, scan page). */
export function followScheduled() {
  listen("job-start", (p) => {
    if (job.running) return;
    Object.assign(job, { scheduled: true, kind: p.kind || "scan", title: p.title || "استخراج زمان‌بندی‌شده", running: true, status: "در حال شروع…", progress: null, sending: null, captcha: "", logs: [], found: [], result: null });
    notify();
  });
  // only for a job the scheduler started — never touch the user's own running job
  listen("job-event", (e) => job.running && job.scheduled && onEvent(e));
  listen("job-end", (p) => {
    if (!job.scheduled) return;
    job.scheduled = false;
    if (p.skipped) {
      Object.assign(job, { running: false, status: "" });
      return notify();
    }
    Object.assign(job, { running: false, captcha: "", result: p, status: p.message });
    toast(p.message, p.error ? "bad" : "ok");
    notify();
  });
}

export async function runJob(kind, title, cmd, args) {
  if (job.running) {
    toast("یک عملیات دیگر در حال اجراست؛ ابتدا آن را متوقف کنید.", "bad");
    return null;
  }
  Object.assign(job, { scheduled: false, kind, title, running: true, status: "در حال شروع…", progress: null, sending: null, captcha: "", logs: [], found: [], result: null });
  notify();
  const ch = newChannel();
  ch.onmessage = onEvent;
  try {
    job.result = await invoke(cmd, { ...args, onEvent: ch });
    toast(job.result.message, job.result.error ? "bad" : "ok");
  } catch (e) {
    job.result = { error: true, message: String(e) };
    toast(String(e), "bad");
  } finally {
    job.running = false;
    job.captcha = "";
    job.status = job.result?.message || "";
    notify();
  }
  return job.result;
}

export const stopJob = () => invoke("job_stop");

export function clearResult() {
  job.result = null;
  notify();
}

/** The page was reloaded while a job kept running in the backend: show it and follow it until it ends. */
export function adoptRunning(kind) {
  Object.assign(job, { scheduled: true, kind, title: kind === "scan" ? "استخراج" : "کمپین", running: true, status: "این عملیات قبل از بارگذاری دوباره صفحه شروع شده و هنوز در حال اجراست.", progress: null, sending: null, captcha: "", logs: [], found: [], result: null });
  notify();
  const poll = setInterval(async () => {
    if (job.running && (await invoke("job_current").catch(() => null))) return;
    clearInterval(poll);
    if (!job.running) return; // the scheduler's job-end already reported the real result
    job.scheduled = false;
    Object.assign(job, { running: false, result: { message: "عملیات تمام شد. نتیجه در تاریخچه ثبت شده است." } });
    notify();
  }, 3000);
}
