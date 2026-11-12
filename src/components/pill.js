// Годинник і дата на острові.

const timeFormatter = new Intl.DateTimeFormat("uk-UA", {
  hour: "2-digit",
  minute: "2-digit",
});

const dateFormatter = new Intl.DateTimeFormat("uk-UA", {
  weekday: "long",
  day: "numeric",
  month: "long",
});

/**
 * Запускає годинник у всіх елементах `clockEls` і дату в `dateEl`.
 * Оновлюємо рівно на початку кожної хвилини, а не кожну секунду.
 */
export function startClock(clockEls, dateEl) {
  const tick = () => {
    const now = new Date();
    const time = timeFormatter.format(now);
    for (const el of clockEls) {
      el.textContent = time;
      el.dateTime = now.toISOString();
    }
    if (dateEl) dateEl.textContent = dateFormatter.format(now);

    // Скільки мілісекунд лишилося до наступної хвилини
    const msToNextMinute = 60_000 - (now.getSeconds() * 1000 + now.getMilliseconds());
    setTimeout(tick, msToNextMinute);
  };
  tick();
}
