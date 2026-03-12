import { scrapeContent as searchScrape } from "../../scraper/eporner/epornerSearchController";
import { scrapeContent as videoScrape } from "../../scraper/eporner/epornerGetController";
import c from "../../utils/options";
import { logger } from "../../utils/logger";
import { maybeError } from "../../utils/modifier";
import { Request, Response } from "express";
import LustPress from "../../LustPress";

const lust = new LustPress();

export async function randomEporner(req: Request, res: Response) {
  /**
   * @api {get} /eporner/random Get random eporner
   * @apiName Get random eporner
   * @apiGroup eporner
   * @apiDescription Get a random eporner video
   *
   * @apiParam {String} id Video ID
   *
   * @apiSuccessExample {json} Success-Response:
   *   HTTP/1.1 200 OK
   *   HTTP/1.1 400 Bad Request
   *
   * @apiExample {curl} curl
   * curl -i https://lust.scathach.id/eporner/random
   *
   * @apiExample {js} JS/TS
   * import axios from "axios"
   *
   * axios.get("https://lust.scathach.id/eporner/random")
   * .then(res => console.log(res.data))
   * .catch(err => console.error(err))
   *
   * @apiExample {python} Python
   * import aiohttp
   * async with aiohttp.ClientSession() as session:
   *  async with session.get("https://lust.scathach.id/eporner/random") as resp:
   *    print(await resp.json())
   */

  // cat/all/SORT-top-weekly/

  try {
    const weeklyUrl = `${c.EPORNER}/cat/all/SORT-top-weekly/`;
    const list = await searchScrape(weeklyUrl);

    if (!list.data.length) {
      throw new Error("No weekly top videos found");
    }

    const random = list.data[Math.floor(Math.random() * list.data.length)];

    let path: string;

    if (random.id.startsWith("video-")) {
      path = random.id;
    } else {
      path = `hd-porn/${random.id}`;
    }

    const url = `${c.EPORNER}/${path}`;
    const data = await videoScrape(url);

    logger.info({
      path: req.path,
      method: req.method,
      ip: req.ip,
      useragent: req.get("User-Agent"),
    });

    return res.json(data);
  } catch (err) {
    res.status(400).json(maybeError(false, (err as Error).message));
  }
}