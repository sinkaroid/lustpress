/**
 * Auto space on url
 * @param str the string to be spaced
 * @returns string
 */
export function spacer(str: string) {
  return str.replace(/\s/g, "+");
}

/**
 * Error handler
 * @param success when success is false, it will return error
 * @param message error message
 * @returns object
 */
export function maybeError(success: boolean, message: string) {
  return { success, message };
}

export function timeAgo(input: Date) {
  const date = new Date(input);
  const formatter: any = new Intl.RelativeTimeFormat("en");
  const ranges: { [key: string]: number } = {
    years: 3600 * 24 * 365,
    months: 3600 * 24 * 30,
    weeks: 3600 * 24 * 7,
    days: 3600 * 24,
    hours: 3600,
    minutes: 60,
    seconds: 1
  };
  const secondsElapsed = (date.getTime() - Date.now()) / 1000;
  for (const key in ranges) {
    if (ranges[key] < Math.abs(secondsElapsed)) {
      const delta = secondsElapsed / ranges[key];
      return formatter.format(Math.round(delta), key);
    }
  }
}