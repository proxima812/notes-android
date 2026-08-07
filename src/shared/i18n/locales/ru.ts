/**
 * Russian — the source dictionary.
 *
 * Its keys define `StringKey`, so every other locale is a `Dictionary` and the
 * compiler refuses a translation that forgets a line or invents one. That is the
 * whole reason the dictionaries are TypeScript and not JSON: a missing key in a
 * language nobody on the team reads would otherwise ship as a blank button.
 *
 * Placeholders are `{name}` and are substituted by `t`. Word order differs
 * wildly across these eight languages, so a phrase is always one string with
 * holes in it rather than sentences glued together in the components.
 *
 * Note counts are written as "заметок: 20" instead of an agreeing plural. Six of
 * these languages have plural rules that do not match Russian, and the label
 * form is correct in all of them — which beats a plural engine nobody can check.
 */
export const ru = {
  "common.back": "Назад",
  "common.loading": "Загрузка…",
  "common.untitled": "Без названия",

  "library.tabActive": "Заметки",
  "library.tabArchived": "Архив",
  "library.search": "Поиск",
  "library.searchInProgress": "Поиск…",
  "library.nothingFound": "Ничего не найдено.",
  "library.archiveEmpty": "Архив пуст.",
  "library.empty": "Пока пусто. Создайте первую заметку.",
  "library.newNote": "Новая заметка",
  "library.templates": "Шаблоны",

  "card.open": "Открыть заметку «{title}»",
  "card.archive": "В архив",
  "card.unarchive": "Вернуть из архива",
  "card.trash": "В корзину",

  "editor.notFound": "Заметка не найдена.",
  "editor.saving": "Сохранение…",
  "editor.saved": "Сохранено",
  "editor.titlePlaceholder": "Заголовок",
  "editor.bodyPlaceholder": "Текст заметки…",
  "editor.color": "Цвет заметки",
  "editor.noColor": "Без цвета",

  "format.bold": "Полужирный",
  "format.italic": "Курсив",
  "format.h2": "Заголовок 2",
  "format.h3": "Заголовок 3",
  "format.quote": "Цитата",
  "format.bullet": "Список",
  "format.ordered": "Нумерованный список",

  "reminder.title": "Напоминание",
  "reminder.close": "Закрыть напоминание",
  "reminder.loading": "Загрузка напоминания…",
  "reminder.name": "Название",
  "reminder.date": "Дата",
  "reminder.time": "Время",
  "reminder.sound": "Звук",
  "reminder.defaultSound": "По умолчанию · {label}",
  "reminder.soundTitle": "Звук уведомления",
  "reminder.soundClose": "Закрыть выбор звука",
  "reminder.soundPresets": "Звуки приложения",
  "reminder.soundDevice": "Звуки телефона",
  "reminder.soundCustom": "Мои звуки",
  "reminder.soundAdd": "Добавить из файла…",
  "reminder.soundAddHint": "Длинный файл будет обрезан до 10 секунд.",
  "reminder.soundMissing": "Звук недоступен",
  "reminder.soundDelete": "Удалить звук «{label}»",
  "reminder.delay": "Android может доставить это напоминание с небольшой задержкой.",
  "reminder.save": "Сохранить напоминание",
  "reminder.delete": "Удалить",
  "reminder.errorWhen": "Укажите дату и время.",
  "reminder.errorPast": "Указанное время уже прошло.",
  "reminder.errorTitle": "Добавьте название напоминания.",
  "reminder.presets": "Готовое время",
  "reminder.presetsPlaceholder": "Выбрать…",
  "reminder.presetsEdit": "Изменить шаблоны времени",
  "reminder.presetsDone": "Готово",
  "reminder.presetsEmpty": "Шаблонов нет — добавьте свой.",
  "reminder.presetAdd": "Добавить",
  "reminder.presetRemove": "Удалить шаблон {time}",

  "reminder.add": "Добавить напоминание",
  "reminder.stopEditing": "Не изменять",
  "reminder.removeOne": "Удалить напоминание «{title}»",

  "reminder.repeat": "Повтор",
  "reminder.repeatNever": "Не повторять",
  "reminder.repeat.daily": "Каждый день",
  "reminder.repeat.weekdays": "По будням",
  "reminder.repeat.weekly": "Каждую неделю",
  "reminder.repeat.monthly": "Каждый месяц",
  "reminder.repeat.yearly": "Каждый год",

  "checklist.title": "Чек-лист",
  "checklist.add": "Новый пункт",
  "checklist.progress": "{done} из {total}",
  "checklist.remove": "Удалить пункт «{title}»",
  "checklist.open": "Чек-лист заметки",

  "filing.title": "Теги",
  "filing.newTag": "Новый тег",
  "filing.addTag": "Создать тег",
  "filing.open": "Теги заметки",
  "filing.all": "Все",

  "appIcon.title": "Иконка приложения",
  "appIcon.hint": "Приложение закроется, чтобы система перерисовала иконку на рабочем столе.",

  "backup.title": "Резервная копия",
  "backup.description": "Копия хранится только там, где вы её положите. Приложение ничего никуда не отправляет.",
  "backup.export": "Сохранить копию",
  "backup.exporting": "Сохраняем…",
  "backup.import": "Восстановить из копии",
  "backup.importing": "Восстанавливаем…",
  "backup.confirmTitle": "Заменить всё содержимое?",
  "backup.confirmBody": "Текущие заметки и напоминания будут заменены содержимым копии. Вернуть их будет нечем.",
  "backup.confirm": "Заменить",
  "backup.cancel": "Отмена",
  "backup.done": "Готово. Заметок: {count}",
  "backup.cancelled": "Ничего не изменилось.",
  "backup.last": "Последняя копия: {name}, {date}",

  "templates.title": "Шаблоны",
  "templates.dialog": "Шаблоны заметок",
  "templates.close": "Закрыть шаблоны",

  "theme.title": "Тема оформления",
  "theme.appearance": "Оформление",
  "theme.close": "Закрыть выбор темы",
  "theme.mint": "Мята",
  "theme.indigo": "Индиго",
  "theme.amethyst": "Аметист",
  "theme.amber": "Янтарь",
  "theme.obsidian": "Обсидиан",

  "color.sunset": "Закат",
  "color.ocean": "Океан",
  "color.forest": "Лес",
  "color.lavender": "Лаванда",
  "color.rose": "Роза",
  "color.amber": "Янтарь",
  "color.mint": "Мята",
  "color.graphite": "Графит",

  "settings.title": "Настройки",
  "settings.build": "{name} {version} · ядро · схема v{schema} · {platform} · заметок: {count}",

  "language.title": "Язык",
  "language.close": "Закрыть выбор языка",

  "error.core": "Ядро приложения не отвечает. Попробуйте перезапустить приложение.",
  "error.unknown": "Произошла неизвестная ошибка.",

  "template.groceries.label": "Закупка продуктов",
  "template.groceries.hint": "Список по отделам магазина",
  "template.groceries.produce": "Овощи и фрукты",
  "template.groceries.dairy": "Молочное и яйца",
  "template.groceries.meat": "Мясо и рыба",
  "template.groceries.pantry": "Бакалея",
  "template.groceries.household": "Бытовое",
  "template.groceries.budget": "Бюджет",
  "template.groceries.budgetHint": "Ориентир: ",

  "template.day.label": "План на день",
  "template.day.hint": "Главное, задачи и заметки",
  "template.day.focus": "Главное за день",
  "template.day.tasks": "Задачи",
  "template.day.meetings": "Встречи",
  "template.day.notes": "Заметки",

  "template.meeting.label": "Встреча",
  "template.meeting.hint": "Повестка, решения, задачи",
  "template.meeting.people": "Участники",
  "template.meeting.agenda": "Повестка",
  "template.meeting.decisions": "Решения",
  "template.meeting.tasks": "Задачи",

  "template.trip.label": "Поездка",
  "template.trip.hint": "Что взять и что не забыть",
  "template.trip.documents": "Документы и деньги",
  "template.trip.clothes": "Одежда",
  "template.trip.tech": "Техника",
  "template.trip.meds": "Аптечка",
  "template.trip.before": "Перед выходом",

  "template.recipe.label": "Рецепт",
  "template.recipe.hint": "Ингредиенты и шаги",
  "template.recipe.ingredients": "Ингредиенты",
  "template.recipe.steps": "Приготовление",
  "template.recipe.notes": "Заметки",

  "template.project.label": "Проект",
  "template.project.hint": "Цель, шаги, риски",
  "template.project.goal": "Цель",
  "template.project.steps": "Шаги",
  "template.project.risks": "Риски",
  "template.project.links": "Ссылки",
} as const;

export type StringKey = keyof typeof ru;

export type Dictionary = Record<StringKey, string>;
