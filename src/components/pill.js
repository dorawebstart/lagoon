// Годинник і дата на острові.

let locale = "uk-UA";
let clockEls = [];
let dateEls = [];
let timer = null;

function render() {
  const now = new Date();
  const time = new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(now);
  for (const el of clockEls) {
    el.textContent = time;
    el.dateTime = now.toISOString();
  }
  const date = new Intl.DateTimeFormat(locale, {
    weekday: "long",
    day: "numeric",
    month: "long",
  }).format(now);
  for (const el of dateEls) el.textContent = date;
  return now;
}

/**
 * Запускає годинник у всіх елементах `clocks` і дату в усіх `dates`.
 * Оновлюємо рівно на початку кожної хвилини, а не кожну секунду.
 */
export function startClock(clocks, dates) {
  clockEls = [...clocks];
  dateEls = [...dates];

  const tick = () => {
    const now = render();
    // Скільки мілісекунд лишилося до наступної хвилини
    const msToNextMinute = 60_000 - (now.getSeconds() * 1000 + now.getMilliseconds());
    timer = setTimeout(tick, msToNextMinute);
  };
  clearTimeout(timer);
  tick();
}

/** Змінює мову дати/часу ("uk-UA", "en-GB") і одразу перемальовує. */
export function setClockLocale(newLocale) {
  locale = newLocale;
  render();
}
