export type Category = 'us_market' | 'kospi_plan';

export type Note = {
  id: string | number;
  title: string;
  content: string;
  category: Category | string;
  created_at?: string;
};
