// Пігулка: поки що лише годинник ГГ:ХХ.

const formatter = new Intl.DateTimeFormat("uk-UA", {
  hour: "2-digit",
  minute: "2-digit",
});

/**
 * Запускає годинник в елементі `clockEl`.
 * Оновлюємо рівно на початку кожної хвилини, а не кожну секунду.
 */
export function startClock(clockEl) {
  const tick = () => {
    const now = new Date();
    clockEl.textContent = formatter.format(now);
    clockEl.dateTime = now.toISOString();

    // Скільки мілісекунд лишилося до наступної хвилини
    const msToNextMinute = 60_000 - (now.getSeconds() * 1000 + now.getMilliseconds());
    setTimeout(tick, msToNextMinute);
  };
  tick();
}
